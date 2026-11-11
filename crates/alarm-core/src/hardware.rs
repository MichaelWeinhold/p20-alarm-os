use std::fmt;

use crate::time::UnixSeconds;

#[derive(Debug, PartialEq, Eq)]
pub enum HardwareError {
    Absent(&'static str),
    Failed {
        device: &'static str,
        reason: String,
    },
}

impl fmt::Display for HardwareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent(device) => write!(formatter, "{device} is not present on this device"),
            Self::Failed { device, reason } => write!(formatter, "{device} failed: {reason}"),
        }
    }
}

impl std::error::Error for HardwareError {}

pub trait Clock {
    fn now(&self) -> Result<UnixSeconds, HardwareError>;
}

pub trait WakeAlarm {
    fn arm(&mut self, at: UnixSeconds) -> Result<(), HardwareError>;
    fn disarm(&mut self) -> Result<(), HardwareError>;
}

pub trait Suspend {
    fn suspend(&mut self) -> Result<(), HardwareError>;
}

pub trait Vibrator {
    fn vibrate(&mut self, milliseconds: u32) -> Result<(), HardwareError>;
}
