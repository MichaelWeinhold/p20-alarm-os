use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

use crate::block::{BLOCKS, named_device, partition};
use crate::kmsg;

pub const MS_RDONLY: c_ulong = 0x1;
pub const MS_NOSUID: c_ulong = 0x2;
pub const MS_NODEV: c_ulong = 0x4;
pub const MS_NOATIME: c_ulong = 0x400;

const SYS_REBOOT: c_long = 142;
const REBOOT_MAGIC1: c_long = 0xfee1_dead;
const REBOOT_MAGIC2: c_long = 0x2812_1969;
const REBOOT_RESTART2: c_long = 0xa1b2_c3d4;
const REBOOT_POWER_OFF: c_long = 0x4321_fedc;
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
    fn sync();
    fn setsid() -> c_int;
    fn ioctl(descriptor: c_int, request: c_int, ...) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
}

pub fn mounted(filesystem: &CStr, target: &CStr) -> io::Result<()> {
    mounted_from(filesystem, target, filesystem, 0, None)
}

pub fn mounted_from(
    source: &CStr,
    target: &CStr,
    filesystem: &CStr,
    flags: c_ulong,
    data: Option<&CStr>,
) -> io::Result<()> {
    let data = data.map_or(std::ptr::null(), |data| data.as_ptr().cast::<c_void>());
    let result = unsafe {
        mount(
            source.as_ptr(),
            target.as_ptr(),
            filesystem.as_ptr(),
            flags,
            data,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[must_use]
pub fn named(name: &str) -> Option<String> {
    fs::read_dir(BLOCKS)
        .ok()?
        .flatten()
        .find_map(|entry| partition(&fs::read_to_string(entry.path().join("uevent")).ok()?, name))
}

pub fn named_devices() -> io::Result<Vec<(String, String)>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir(BLOCKS)? {
        if let Some(device) = named_device(&fs::read_to_string(entry?.path().join("uevent"))?) {
            devices.push(device);
        }
    }
    Ok(devices)
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

#[must_use]
pub fn powered_off() -> io::Error {
    unsafe {
        sync();
        syscall(
            SYS_REBOOT,
            REBOOT_MAGIC1,
            REBOOT_MAGIC2,
            REBOOT_POWER_OFF,
            0,
        );
    }
    io::Error::last_os_error()
}

#[must_use]
pub fn rebooted_into_recovery() -> io::Error {
    unsafe {
        sync();
    }
    reboot_into(c"recovery")
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
