use std::collections::BTreeSet;
use std::ffi::{c_int, c_long, c_void};
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::FileExt;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::{
    Options, REGISTERS, Stop, argument, argument_size, entry, hifi_descriptor, hifi_request,
    ioctl_entered, stop,
};

const PTRACE_SYSCALL: c_int = 24;
const PTRACE_GETREGSET: c_int = 0x4204;
const PTRACE_SEIZE: c_int = 0x4206;
const PTRACE_INTERRUPT: c_int = 0x4207;
const PTRACE_O_TRACESYSGOOD: usize = 1;
const PTRACE_O_TRACECLONE: usize = 8;
const NT_PRSTATUS: usize = 1;
const EPERM: i32 = 1;
const ESRCH: i32 = 3;
const WALL: c_int = 0x4000_0000;
const POLL: Duration = Duration::from_micros(200);

unsafe extern "C" {
    fn ptrace(request: c_int, ...) -> c_long;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
}

#[repr(C)]
struct Iovec {
    base: *mut c_void,
    length: usize,
}

fn checked(result: c_long) -> io::Result<()> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn seized(tid: c_int) -> io::Result<()> {
    let options = PTRACE_O_TRACESYSGOOD | PTRACE_O_TRACECLONE;
    checked(unsafe { ptrace(PTRACE_SEIZE, tid, 0usize, options) })?;
    checked(unsafe { ptrace(PTRACE_INTERRUPT, tid, 0usize, 0usize) })
}

fn resumed(tid: c_int, signal: i32) -> io::Result<()> {
    let signal = usize::try_from(signal).unwrap_or(0);
    checked(unsafe { ptrace(PTRACE_SYSCALL, tid, 0usize, signal) })
}

fn registers(tid: c_int) -> io::Result<[u64; REGISTERS]> {
    let mut registers = [0u64; REGISTERS];
    let mut vector = Iovec {
        base: registers.as_mut_ptr().cast::<c_void>(),
        length: size_of_val(&registers),
    };
    checked(unsafe { ptrace(PTRACE_GETREGSET, tid, NT_PRSTATUS, &raw mut vector) })?;
    Ok(registers)
}

fn waited() -> io::Result<(c_int, i32)> {
    let mut status: c_int = 0;
    let tid = unsafe { waitpid(-1, &raw mut status, WALL) };
    if tid < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok((tid, status))
    }
}

fn watched(deadline: Instant, count: usize, recorded: Arc<AtomicUsize>) {
    thread::spawn(move || {
        thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let recorded = recorded.load(Ordering::SeqCst);
        eprintln!("dsp-recorder: {recorded} of {count} ioctls recorded in time");
        process::exit(1);
    });
}

fn memory(tid: c_int, at: u64, length: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0u8; length];
    File::open(format!("/proc/{tid}/mem"))?.read_exact_at(&mut bytes, at)?;
    Ok(bytes)
}

fn started(program: &str, deadline: Instant) -> io::Result<c_int> {
    while Instant::now() < deadline {
        for process in fs::read_dir("/proc")?.flatten() {
            let Ok(pid) = process.file_name().to_string_lossy().parse::<c_int>() else {
                continue;
            };
            let Ok(command) = fs::read(process.path().join("cmdline")) else {
                continue;
            };
            if command.split(|byte| *byte == 0).next() == Some(program.as_bytes()) {
                return Ok(pid);
            }
        }
        thread::sleep(POLL);
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("{program} did not start"),
    ))
}

fn threads(pid: c_int) -> io::Result<BTreeSet<c_int>> {
    let mut found = BTreeSet::new();
    for task in fs::read_dir(format!("/proc/{pid}/task"))? {
        if let Ok(tid) = task?.file_name().to_string_lossy().parse() {
            found.insert(tid);
        }
    }
    Ok(found)
}

fn already_traced_or_gone(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(EPERM | ESRCH))
}

fn attached(pid: c_int) -> io::Result<()> {
    seized(pid)?;
    let mut traced = BTreeSet::from([pid]);
    loop {
        let fresh: Vec<c_int> = threads(pid)?.difference(&traced).copied().collect();
        if fresh.is_empty() {
            return Ok(());
        }
        for tid in fresh {
            match seized(tid) {
                Err(error) if !already_traced_or_gone(&error) => return Err(error),
                _ => {}
            }
            traced.insert(tid);
        }
    }
}

fn hifi_entry(tid: c_int) -> io::Result<Option<Vec<u8>>> {
    let Some(ioctl) = ioctl_entered(&registers(tid)?) else {
        return Ok(None);
    };
    if !hifi_request(ioctl.request) {
        return Ok(None);
    }
    let target = fs::read_link(format!("/proc/{tid}/fd/{}", ioctl.descriptor))?;
    if !hifi_descriptor(&target) {
        return Ok(None);
    }
    let found = argument(&memory(tid, ioctl.argument, argument_size(ioctl.request))?)?;
    let length = usize::try_from(found.length)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let body = memory(tid, found.body, length)?;
    entry(ioctl.request, found.reply, &body).map(Some)
}

pub fn record(options: &Options) -> io::Result<usize> {
    let deadline = Instant::now() + options.within;
    let mut recording = File::create(&options.recording)?;
    let pid = started(&options.program, deadline)?;
    let shared = Arc::new(AtomicUsize::new(0));
    watched(deadline, options.count, Arc::clone(&shared));
    attached(pid)?;
    let mut recorded = 0;
    while recorded < options.count {
        let (tid, status) = waited()?;
        match stop(status) {
            Stop::Gone if tid == pid => {
                return Err(io::Error::other(format!(
                    "the program ended after {recorded} ioctls"
                )));
            }
            Stop::Gone => {}
            Stop::Syscall => {
                if let Some(entry) = hifi_entry(tid)? {
                    recording.write_all(&entry)?;
                    recorded += 1;
                    shared.store(recorded, Ordering::SeqCst);
                }
                resumed(tid, 0)?;
            }
            Stop::Quiet => resumed(tid, 0)?,
            Stop::Signal(signal) => resumed(tid, signal)?,
        }
    }
    recording.sync_all()?;
    Ok(recorded)
}
