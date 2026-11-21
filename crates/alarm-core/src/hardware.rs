use std::fmt;

use crate::battery::BatteryReading;
use crate::boot::Boot;
use crate::face::{Frame, Size};
use crate::input::InputEvent;
use crate::settings::{RECORD_BYTES, SLOT_BYTES, Slot};
use crate::time::UnixSeconds;

#[derive(Clone, Debug, PartialEq, Eq)]
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

pub trait PowerOff {
    fn power_off(&mut self, on_at: Option<UnixSeconds>) -> Result<(), HardwareError>;
}

pub trait BootReason {
    fn read(&self) -> Result<Boot, HardwareError>;
}

pub trait Vibrator {
    fn vibrate(&mut self, milliseconds: u32) -> Result<(), HardwareError>;
    fn stop(&mut self) -> Result<(), HardwareError>;
}

pub trait Speaker {
    fn start(&mut self) -> Result<(), HardwareError>;
    fn stop(&mut self) -> Result<(), HardwareError>;
}

pub trait Indicator {
    fn warn(&mut self) -> Result<(), HardwareError>;
    fn clear(&mut self) -> Result<(), HardwareError>;
}

pub trait Battery {
    fn read(&self) -> Result<BatteryReading, HardwareError>;
    fn charger_enabled(&self) -> Result<bool, HardwareError>;
    fn set_charger_enabled(&mut self, enabled: bool) -> Result<(), HardwareError>;
}

pub trait Display {
    fn size(&self) -> Size;
    fn show(&mut self, frame: &Frame) -> Result<(), HardwareError>;
    fn light(&mut self, on: bool) -> Result<(), HardwareError>;
}

pub trait Input {
    fn wait(&mut self, seconds: u64) -> Result<Option<InputEvent>, HardwareError>;
}

pub trait Settings {
    fn load(&mut self) -> Result<[u8; RECORD_BYTES], HardwareError>;
    fn save(&mut self, slot: Slot, bytes: &[u8; SLOT_BYTES]) -> Result<(), HardwareError>;
}

pub trait Modem {
    fn boot(&mut self) -> Result<(), HardwareError>;
    fn radio_off(&mut self) -> Result<(), HardwareError>;
}
