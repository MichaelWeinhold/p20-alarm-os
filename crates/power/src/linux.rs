use std::ffi::{c_int, c_long};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use alarm_core::hardware::{HardwareError, PowerOff, Recovery};
use alarm_core::time::UnixSeconds;
use pid1::linux::{powered_off, rebooted_into_recovery};

use crate::{failed, timer_target};

const CLOCK_POWEROFF_ALARM: c_int = 12;
const TFD_TIMER_ABSTIME: c_int = 1;
const EINVAL: i32 = 22;
const TIMER_DEVICE: &str = "poweroff alarm";
const REBOOT_DEVICE: &str = "reboot";

#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: c_long,
}

#[repr(C)]
struct Itimerspec {
    interval: Timespec,
    value: Timespec,
}

unsafe extern "C" {
    fn timerfd_create(clock: c_int, flags: c_int) -> c_int;
    fn timerfd_settime(
        descriptor: c_int,
        flags: c_int,
        new: *const Itimerspec,
        old: *mut Itimerspec,
    ) -> c_int;
}

fn created() -> io::Result<OwnedFd> {
    let descriptor = unsafe { timerfd_create(CLOCK_POWEROFF_ALARM, 0) };
    if descriptor < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }
}

fn set(timer: &OwnedFd, seconds: i64) -> io::Result<()> {
    let spec = Itimerspec {
        interval: Timespec {
            seconds: 0,
            nanoseconds: 0,
        },
        value: Timespec {
            seconds,
            nanoseconds: 0,
        },
    };
    let result = unsafe {
        timerfd_settime(
            timer.as_raw_fd(),
            TFD_TIMER_ABSTIME,
            &raw const spec,
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[derive(Debug)]
pub struct PoweroffAlarm(OwnedFd);

impl PoweroffAlarm {
    pub fn open() -> Result<Self, HardwareError> {
        created().map(Self).map_err(|error| {
            if error.raw_os_error() == Some(EINVAL) {
                HardwareError::Absent(TIMER_DEVICE)
            } else {
                failed(TIMER_DEVICE, &error)
            }
        })
    }
}

impl PowerOff for PoweroffAlarm {
    fn power_off(&mut self, on_at: Option<UnixSeconds>) -> Result<(), HardwareError> {
        let seconds = on_at.map_or(0, |at| timer_target(at).0);
        set(&self.0, seconds).map_err(|error| failed(TIMER_DEVICE, &error))?;
        Err(failed(REBOOT_DEVICE, &powered_off()))
    }
}

#[derive(Debug)]
pub struct RecoveryReboot;

impl Recovery for RecoveryReboot {
    fn reboot_into_recovery(&mut self) -> Result<(), HardwareError> {
        Err(failed(REBOOT_DEVICE, &rebooted_into_recovery()))
    }
}
