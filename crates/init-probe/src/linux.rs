use std::fs::{self, File};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use alarm_core::battery::{ChargeLimit, stop_at_limit};
use battery::SysfsBattery;
use pid1::kmsg;
use pid1::linux::{into_recovery, mounted};

use crate::marker;
use crate::power::{self, Step};

fn cmdline() -> Vec<String> {
    match mounted(c"proc", c"/proc").and_then(|()| fs::read_to_string("/proc/cmdline")) {
        Ok(text) => marker::cmdline(&text),
        Err(error) => vec![kmsg::failure("reading /proc/cmdline", &error)],
    }
}

fn logged() -> io::Result<File> {
    mounted(c"devtmpfs", c"/dev")?;
    let mut kmsg = pid1::linux::kmsg()?;
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
        return kmsg.write_all(kmsg::failure("mounting /sys", &error).as_bytes());
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
                    kmsg.write_all(kmsg::failure(&step, &error).as_bytes())?;
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
    let mut kmsg = logged().and_then(|mut kmsg| sampled(&mut kmsg).map(|()| kmsg));
    into_recovery(kmsg.as_mut().ok())
}
