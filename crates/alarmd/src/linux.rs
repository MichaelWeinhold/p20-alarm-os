use std::fs::File;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use alarm_core::hardware::{Display, HardwareError};
use alarm_core::light::Light;
use alarm_core::ringer::listen;
use alarm_core::time::{TimeOfDay, UtcOffset};
use alarm_core::window::Window;
use battery::SysfsBattery;
use clock::SystemClock;
use framebuffer::linux::Panel;
use input::linux::{Answer, EvdevInput, FirmwareLoader};
use pid1::kmsg;
use pid1::linux::{into_recovery, mounted};
use power::Cmdline;
use power::linux::PoweroffAlarm;
use storage::Partition;
use suspend::{SysfsSuspend, SysfsWakeAlarm};

use crate::{
    ALARM_HOUR, ALARM_MINUTE, BRIGHTNESS, Booted, LOCAL_OFFSET_SECONDS, Restored, Sleep, booted,
    cable_in, change, choices, minute, persisted, rest, restored,
};

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

fn restoring(log: &mut Option<File>) -> (Result<Partition, HardwareError>, Restored) {
    let offset = match UtcOffset::from_seconds(LOCAL_OFFSET_SECONDS) {
        Ok(offset) => offset,
        Err(error) => {
            logged(log, &kmsg::failure("reading the UTC offset", &error));
            into_recovery(log.as_mut())
        }
    };
    let alarm = match TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE) {
        Ok(alarm) => alarm,
        Err(error) => {
            logged(log, &kmsg::failure("reading the alarm time", &error));
            into_recovery(log.as_mut())
        }
    };
    let mut settings = Partition::open();
    let restored = restored(&mut settings, &SystemClock, alarm, offset);
    for line in &restored.log {
        logged(log, line);
    }
    (settings, restored)
}

fn booting(log: &mut Option<File>) -> Light {
    let Booted { light, log: lines } = booted(&Cmdline::open(), &SystemClock);
    for line in &lines {
        logged(log, line);
    }
    light
}

pub fn run() -> ! {
    let mut log = started().ok();
    let (
        mut settings,
        Restored {
            mut saved,
            mut ringer,
            ..
        },
    ) = restoring(&mut log);
    let offset = saved.offset;
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
    let mut light = booting(&mut log);
    let mut battery = SysfsBattery::open();
    let mut sleep = Sleep {
        window: Window::lit(saved.window_seconds),
        wake: SysfsWakeAlarm::open(),
        suspend: SysfsSuspend::open(),
        power: PoweroffAlarm::open(),
    };
    let mut input = EvdevInput::open();
    let mut absent = Vec::new();
    let mut was_lit = false;
    loop {
        let before = ringer;
        let turn = minute(
            &SystemClock,
            offset,
            &mut battery,
            &mut ringer,
            &mut light,
            &mut panel,
        );
        if light.lit() != was_lit {
            let state = if light.lit() { "lit" } else { "off" };
            logged(&mut log, &kmsg::line(&format!("panel {state}")));
            was_lit = light.lit();
            let current = choices(&ringer, offset, saved.window_seconds);
            if !was_lit && let Some(line) = persisted(&mut settings, &mut saved, current) {
                logged(&mut log, &line);
            }
        }
        for line in &turn.log {
            logged(&mut log, line);
        }
        let rested = rest(
            &SystemClock,
            &ringer,
            &light,
            cable_in(&battery),
            &mut sleep,
            turn.wait_seconds,
        );
        for line in &rested.log {
            logged(&mut log, line);
        }
        let size = panel.size();
        let heard = listen(
            &mut input,
            rested.wait_seconds,
            &SystemClock,
            offset,
            size,
            &mut ringer,
            &mut light,
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
