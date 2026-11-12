use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crate::marker;

const SYS_REBOOT: c_long = 142;
const REBOOT_MAGIC1: c_long = 0xfee1_dead;
const REBOOT_MAGIC2: c_long = 0x2812_1969;
const REBOOT_RESTART2: c_long = 0xa1b2_c3d4;

unsafe extern "C" {
    fn mount(
        source: *const c_char,
        target: *const c_char,
        filesystem: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
    fn syscall(number: c_long, ...) -> c_long;
}

fn mounted(filesystem: &CStr, target: &CStr) -> io::Result<()> {
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

fn cmdline() -> Vec<String> {
    match mounted(c"proc", c"/proc").and_then(|()| fs::read_to_string("/proc/cmdline")) {
        Ok(text) => marker::cmdline(&text),
        Err(error) => vec![marker::failure("reading /proc/cmdline", &error)],
    }
}

fn probe() -> io::Result<()> {
    mounted(c"devtmpfs", c"/dev")?;
    let mut kmsg: File = OpenOptions::new().write(true).open("/dev/kmsg")?;
    kmsg.write_all(marker::marker().as_bytes())?;
    for line in cmdline() {
        kmsg.write_all(line.as_bytes())?;
    }
    let error = reboot_into(c"bootloader");
    kmsg.write_all(marker::failure("rebooting into the bootloader", &error).as_bytes())
}

pub fn run() -> ! {
    let _ = probe();
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
