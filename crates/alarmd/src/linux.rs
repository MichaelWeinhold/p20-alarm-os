use std::fs::File;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use alarm_core::time::UtcOffset;
use battery::SysfsBattery;
use clock::SystemClock;
use framebuffer::linux::Panel;
use pid1::kmsg;
use pid1::linux::{into_recovery, mounted};

use crate::{BRIGHTNESS, LOCAL_OFFSET_SECONDS, minute};

fn logged(log: &mut Option<File>, line: &str) {
    if let Some(file) = log {
        let _ = file.write_all(line.as_bytes());
    }
}

fn started() -> io::Result<File> {
    mounted(c"devtmpfs", c"/dev")?;
    let mut log = pid1::linux::kmsg()?;
    for (filesystem, target) in [(c"proc", c"/proc"), (c"sysfs", c"/sys")] {
        if let Err(error) = mounted(filesystem, target) {
            let step = format!("mounting {}", target.to_string_lossy());
            log.write_all(kmsg::failure(&step, &error).as_bytes())?;
        }
    }
    log.write_all(kmsg::line("alarmd started").as_bytes())?;
    Ok(log)
}

pub fn run() -> ! {
    let mut log = started().ok();
    let offset = match UtcOffset::from_seconds(LOCAL_OFFSET_SECONDS) {
        Ok(offset) => offset,
        Err(error) => {
            logged(&mut log, &kmsg::failure("reading the UTC offset", &error));
            into_recovery(log.as_mut())
        }
    };
    let mut panel = match Panel::open(BRIGHTNESS) {
        Ok(panel) => panel,
        Err(error) => {
            logged(&mut log, &kmsg::failure("opening the panel", &error));
            into_recovery(log.as_mut())
        }
    };
    let mut battery = SysfsBattery::open();
    loop {
        let step = minute(&SystemClock, offset, &mut battery, &mut panel);
        for line in &step.log {
            logged(&mut log, line);
        }
        thread::sleep(Duration::from_secs(step.sleep_seconds));
    }
}
