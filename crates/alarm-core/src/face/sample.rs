use std::fmt;

use super::{Countdown, Face, Listed, Running, Screen};
use crate::alarm::{Alarm, Weekdays};
use crate::battery::{BatteryError, ChargeLimit, Gauge, Percent};
use crate::ringer::Outputs;
use crate::sound::SoundName;
use crate::time::{TimeError, TimeOfDay, Weekday};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleError {
    Time(TimeError),
    Battery(BatteryError),
}

impl fmt::Display for SampleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Time(error) => write!(formatter, "{error}"),
            Self::Battery(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SampleError {}

impl From<TimeError> for SampleError {
    fn from(error: TimeError) -> Self {
        Self::Time(error)
    }
}

impl From<BatteryError> for SampleError {
    fn from(error: BatteryError) -> Self {
        Self::Battery(error)
    }
}

pub fn charged(value: u8) -> Result<Gauge, SampleError> {
    Ok(Gauge::of(Percent::new(value)?, ChargeLimit::DEFAULT))
}

pub fn listed(
    hour: u8,
    minute: u8,
    repeat: Weekdays,
    until: Option<i64>,
) -> Result<Listed, SampleError> {
    Ok(Listed {
        alarm: Alarm {
            time: TimeOfDay::new(hour, minute)?,
            repeat,
        },
        armed: until.is_some(),
        until,
    })
}

#[must_use]
pub fn weekend() -> Weekdays {
    Weekdays::NONE.with(Weekday::Monday).with(Weekday::Sunday)
}

pub fn face(armed: bool, ringing: bool, battery: Gauge) -> Result<Face, SampleError> {
    let until = armed.then_some(11 * 3_600 + 8 * 60);
    Ok(Face {
        time: TimeOfDay::new(19, 37)?,
        weekday: Weekday::Monday,
        day: 18,
        alarms: vec![
            listed(6, 45, Weekdays::EVERY_DAY, until)?,
            listed(7, 30, weekend(), None)?,
        ],
        until,
        editing: None,
        rang: if armed {
            Some(TimeOfDay::new(6, 45)?)
        } else {
            None
        },
        marked: None,
        ringing,
        battery,
        screen: Screen::Alarm,
        power_off: true,
        lit_seconds: 10,
        ramp_seconds: 60,
        outputs: Outputs::BOTH,
        recovery_marked: false,
        timers: Vec::new(),
        countdown: Countdown::default(),
        running: None,
        rung: None,
        volume: None,
        sound: SoundName::new("ringtone.wav"),
        listing: None,
        scroll: 0,
    })
}

pub fn timer(running: Option<Running>, ringing: bool) -> Result<Face, SampleError> {
    Ok(Face {
        screen: Screen::Timer,
        timers: vec![Countdown::new(0, 3, 0)?, Countdown::new(0, 12, 5)?],
        countdown: Countdown::new(0, 3, 0)?,
        running,
        ..face(true, ringing, charged(80)?)?
    })
}

pub fn sheet() -> Result<Face, SampleError> {
    face(true, false, charged(80)?)
}

pub fn timers() -> Result<Face, SampleError> {
    timer(None, false)
}

pub fn settings() -> Result<Face, SampleError> {
    Ok(Face {
        screen: Screen::Settings,
        ramp_seconds: 0,
        ..sheet()?
    })
}
