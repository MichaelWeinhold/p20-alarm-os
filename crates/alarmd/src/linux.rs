use std::fs::File;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use alarm_core::hardware::Display;
use alarm_core::ringer::{Ringer, listen};
use alarm_core::time::{TimeOfDay, UtcOffset};
use battery::SysfsBattery;
use clock::SystemClock;
use framebuffer::linux::Panel;
use input::linux::{Answer, EvdevInput, FirmwareLoader};
use pid1::kmsg;
use pid1::linux::{into_recovery, mounted};

use crate::{ALARM_HOUR, ALARM_MINUTE, BRIGHTNESS, LOCAL_OFFSET_SECONDS, change, minute};

const FIRMWARE_POLL: Duration = Duration::from_millis(250);
const INPUT_RETRY: Duration = Duration::from_secs(1);

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

fn answering_firmware() {
    let mut log = pid1::linux::kmsg().ok();
    let loader = match FirmwareLoader::mount() {
        Ok(loader) => loader,
        Err(error) => {
            logged(&mut log, &kmsg::failure("mounting odm", &error));
            return;
        }
    };
    loop {
        let answers = match loader.answer_pending() {
            Ok(answers) => answers,
            Err(error) => {
                logged(
                    &mut log,
                    &kmsg::failure("reading firmware requests", &error),
                );
                return;
            }
        };
        for Answer { name, outcome } in answers {
            match outcome {
                Ok(()) => logged(&mut log, &kmsg::line(&format!("firmware {name} answered"))),
                Err(error) => logged(
                    &mut log,
                    &kmsg::failure(&format!("answering firmware {name}"), &error),
                ),
            }
        }
        thread::sleep(FIRMWARE_POLL);
    }
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
    let mut ringer = match TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE) {
        Ok(alarm) => Ringer::new(alarm),
        Err(error) => {
            logged(&mut log, &kmsg::failure("reading the alarm time", &error));
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
    if let Err(error) = thread::Builder::new()
        .name("firmware".to_owned())
        .spawn(answering_firmware)
    {
        logged(
            &mut log,
            &kmsg::failure("starting the firmware answers", &error),
        );
    }
    let mut battery = SysfsBattery::open();
    let mut input = EvdevInput::open();
    let mut absent = Vec::new();
    loop {
        let before = ringer;
        let turn = minute(&SystemClock, offset, &mut battery, &mut ringer, &mut panel);
        for line in &turn.log {
            logged(&mut log, line);
        }
        let size = panel.size();
        let heard = listen(
            &mut input,
            turn.wait_seconds,
            &SystemClock,
            offset,
            size,
            &mut ringer,
        );
        if let Some(line) = change(&before, &ringer) {
            logged(&mut log, &line);
        }
        if input.absent() != absent {
            absent = input.absent();
            logged(
                &mut log,
                &kmsg::line(&format!("input devices absent: {absent:?}")),
            );
        }
        if let Err(error) = heard {
            logged(&mut log, &kmsg::failure("reading input", &error));
            thread::sleep(INPUT_RETRY);
        }
    }
}
