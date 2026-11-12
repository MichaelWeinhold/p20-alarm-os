use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use alarm_core::battery::{ChargeLimit, stop_at_limit};
use battery::SysfsBattery;

use crate::marker;
use crate::power::{self, Step};

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

fn logged() -> io::Result<File> {
    mounted(c"devtmpfs", c"/dev")?;
    let mut kmsg: File = OpenOptions::new().write(true).open("/dev/kmsg")?;
    kmsg.write_all(marker::marker().as_bytes())?;
    for line in cmdline() {
        kmsg.write_all(line.as_bytes())?;
    }
    Ok(kmsg)
}

fn node(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| error.to_string())
}

fn interfaces() -> Result<String, String> {
    let entries = fs::read_dir(power::INTERFACES).map_err(|error| error.to_string())?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names.join(","))
}

fn sampled(kmsg: &mut File) -> io::Result<()> {
    if let Err(error) = mounted(c"sysfs", c"/sys") {
        return kmsg.write_all(marker::failure("mounting /sys", &error).as_bytes());
    }
    kmsg.write_all(power::line("net", &[("interfaces", interfaces())]).as_bytes())?;
    let mut battery = SysfsBattery::open();
    for step in power::plan() {
        match step {
            Step::Sample(phase) => {
                let readings: Vec<(&str, Result<String, String>)> = power::READINGS
                    .iter()
                    .map(|(name, path)| (*name, node(path)))
                    .collect();
                kmsg.write_all(power::line(phase, &readings).as_bytes())?;
            }
            Step::Write(path, value) => {
                if let Err(error) = fs::write(path, value) {
                    let step = format!("writing {value} to {path}");
                    kmsg.write_all(marker::failure(&step, &error).as_bytes())?;
                }
            }
            Step::StopCharge => {
                let result = match &mut battery {
                    Ok(battery) => stop_at_limit(ChargeLimit::DEFAULT, battery),
                    Err(error) => Err(error.clone()),
                };
                kmsg.write_all(power::stop(&result).as_bytes())?;
            }
            Step::Wait(seconds) => thread::sleep(Duration::from_secs(seconds)),
        }
    }
    Ok(())
}

pub fn run() -> ! {
    let kmsg = logged().and_then(|mut kmsg| sampled(&mut kmsg).map(|()| kmsg));
    let error = reboot_into(c"recovery");
    if let Ok(mut kmsg) = kmsg {
        let _ = kmsg.write_all(marker::failure("rebooting into recovery", &error).as_bytes());
    }
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
