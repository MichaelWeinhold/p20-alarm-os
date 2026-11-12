use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

use crate::kmsg;

const SYS_REBOOT: c_long = 142;
const REBOOT_MAGIC1: c_long = 0xfee1_dead;
const REBOOT_MAGIC2: c_long = 0x2812_1969;
const REBOOT_RESTART2: c_long = 0xa1b2_c3d4;
const O_NOCTTY: c_int = 0o400;
const TIOCSCTTY: c_int = 0x540e;

unsafe extern "C" {
    fn mount(
        source: *const c_char,
        target: *const c_char,
        filesystem: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
    fn syscall(number: c_long, ...) -> c_long;
    fn setsid() -> c_int;
    fn ioctl(descriptor: c_int, request: c_int, ...) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
}

pub fn mounted(filesystem: &CStr, target: &CStr) -> io::Result<()> {
    let result = unsafe {
        mount(
            filesystem.as_ptr(),
            target.as_ptr(),
            filesystem.as_ptr(),
            0,
            std::ptr::null(),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn kmsg() -> io::Result<File> {
    OpenOptions::new().write(true).open("/dev/kmsg")
}

fn reboot_into(target: &CStr) -> io::Error {
    unsafe {
        syscall(
            SYS_REBOOT,
            REBOOT_MAGIC1,
            REBOOT_MAGIC2,
            REBOOT_RESTART2,
            target.as_ptr(),
        );
    }
    io::Error::last_os_error()
}

pub fn into_recovery(log: Option<&mut File>) -> ! {
    let error = reboot_into(c"recovery");
    if let Some(log) = log {
        let _ = log.write_all(kmsg::failure("rebooting into recovery", &error).as_bytes());
    }
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

pub fn spawned_on(terminal: &Path, program: &str, path: &str) -> io::Result<Child> {
    let terminal = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NOCTTY)
        .open(terminal)?;
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", path)
        .current_dir("/")
        .stdin(Stdio::from(terminal.try_clone()?))
        .stdout(Stdio::from(terminal.try_clone()?))
        .stderr(Stdio::from(terminal));
    unsafe {
        command.pre_exec(|| {
            if setsid() < 0 || ioctl(0, TIOCSCTTY, 0) < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    command.spawn()
}

pub fn reaped(child: &Child) -> io::Result<ExitStatus> {
    let wanted = c_int::try_from(child.id())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    loop {
        let mut status: c_int = 0;
        let pid = unsafe { waitpid(-1, &raw mut status, 0) };
        if pid == wanted {
            return Ok(ExitStatus::from_raw(status));
        }
        if pid < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
