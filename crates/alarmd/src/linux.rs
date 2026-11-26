use std::fs::File;
use std::io::{self, Write};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use alarm_core::hardware::{Display, HardwareError};
use alarm_core::light::Light;
use alarm_core::ringer::{Ringer, listen};
use alarm_core::settings::Choices;
use alarm_core::sound::Sound;
use alarm_core::time::{TimeOfDay, UtcOffset};
use alarm_core::vibration::Vibration;
use alarm_core::warning::Warning;
use alarm_core::window::Window;
use battery::SysfsBattery;
use clock::SystemClock;
use framebuffer::linux::Panel;
use indicator::RedLed;
use input::linux::{Answer, EvdevInput, FirmwareLoader};
use modem::linux::Balong;
use pid1::kmsg;
use pid1::linux::{into_recovery, mounted};
use power::Cmdline;
use power::linux::{PoweroffAlarm, RecoveryReboot};
use speaker::linux::{AlarmSounds, AlsaSpeaker};
use storage::Partition;
use suspend::{SysfsSuspend, SysfsWakeAlarm};
use vibrator::LedVibrator;

use crate::{
    ALARM_HOUR, ALARM_MINUTE, AfterModem, BRIGHTNESS, Booted, Buzz, LOCAL_OFFSET_SECONDS, Lamp,
    Restored, Sleep, booted, cable_in, change, choices, minute, modem_booted, persisted, pulse,
    recover, rest, restored, sounds_listed, warned,
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

fn booting_modem() {
    let mut log = pid1::linux::kmsg().ok();
    for line in &modem_booted(&mut Balong::open()) {
        logged(&mut log, line);
    }
}

fn modem_started(log: &mut Option<File>) -> Option<JoinHandle<()>> {
    thread::Builder::new()
        .name("modem".to_owned())
        .spawn(booting_modem)
        .map_err(|error| logged(log, &kmsg::failure("starting the modem's boot", &error)))
        .ok()
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
    let mut restored = restored(&mut settings, &SystemClock, alarm, offset);
    let listed = sounds_listed(&AlarmSounds, &mut restored.ringer);
    restored.log.extend(listed);
    for line in &restored.log {
        logged(log, line);
    }
    (settings, restored)
}

fn booting(log: &mut Option<File>, lit_seconds: i64) -> Light {
    let Booted { light, log: lines } = booted(&Cmdline::open(), &SystemClock, lit_seconds);
    for line in &lines {
        logged(log, line);
    }
    light
}

fn buzzing(log: &mut Option<File>) -> Buzz<LedVibrator, AlsaSpeaker> {
    let vibrator = LedVibrator::open();
    if let Err(error) = &vibrator {
        logged(log, &kmsg::failure("opening the vibrator", error));
    }
    let speaker = AlsaSpeaker::open();
    if let Err(error) = &speaker {
        logged(log, &kmsg::failure("opening the speaker", error));
    }
    Buzz {
        vibration: Vibration::still(),
        vibrator,
        sound: Sound::quiet(),
        speaker,
    }
}

fn warning(log: &mut Option<File>) -> Lamp<RedLed> {
    let indicator = RedLed::open();
    if let Err(error) = &indicator {
        logged(log, &kmsg::failure("opening the indicator", error));
    }
    Lamp {
        warning: Warning::unset(),
        indicator,
    }
}

fn answered(
    log: &mut Option<File>,
    before: &Ringer,
    ringer: &mut Ringer,
    settings: &mut Result<Partition, HardwareError>,
    saved: &mut Choices,
) {
    if let Some(line) = change(before, ringer) {
        logged(log, &line);
    }
    if ringer.take_recovery() {
        let current = choices(ringer, saved.offset, saved.window_seconds);
        recover(settings, saved, current, &mut RecoveryReboot, &mut |line| {
            logged(log, line);
        });
    }
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
    let modem = modem_started(&mut log);
    let mut light = booting(&mut log, ringer.lit_seconds());
    let mut battery = SysfsBattery::open();
    let mut sleep = Sleep {
        window: Window::lit(saved.window_seconds),
        wake: SysfsWakeAlarm::open(),
        suspend: SysfsSuspend::open().map(|suspend| AfterModem { suspend, modem }),
        power: PoweroffAlarm::open(),
    };
    let mut buzz = buzzing(&mut log);
    let mut lamp = warning(&mut log);
    let mut input = EvdevInput::open();
    let mut absent = Vec::new();
    let mut was_lit = false;
    loop {
        let before = ringer.clone();
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
        let lit = warned(&battery, &mut lamp);
        let pulsed = pulse(&SystemClock, &ringer, &mut buzz, turn.wait_seconds);
        for line in turn.log.iter().chain(&lit).chain(&pulsed.log) {
            logged(&mut log, line);
        }
        let rested = rest(
            &SystemClock,
            &ringer,
            &light,
            cable_in(&battery),
            &mut sleep,
            pulsed.wait_seconds,
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
        answered(&mut log, &before, &mut ringer, &mut settings, &mut saved);
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
