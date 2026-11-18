use std::fs;
use std::io;
use std::path::Path;

use alarm_core::boot::Boot;
use alarm_core::hardware::{BootReason, HardwareError};
use alarm_core::time::UnixSeconds;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const CMDLINE: &str = "/proc/cmdline";
pub const KERNEL_LEAD_SECONDS: i64 = 60;

const CMDLINE_DEVICE: &str = "proc/cmdline";
const RESET_TYPE: &str = "normal_reset_type=";

fn failed(device: &'static str, error: &io::Error) -> HardwareError {
    if error.kind() == io::ErrorKind::NotFound {
        HardwareError::Absent(device)
    } else {
        HardwareError::Failed {
            device,
            reason: error.to_string(),
        }
    }
}

fn unexpected(reason: &str) -> HardwareError {
    HardwareError::Failed {
        device: CMDLINE_DEVICE,
        reason: reason.to_owned(),
    }
}

fn reset_type(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn reason(text: &str) -> Result<Boot, HardwareError> {
    let line = text
        .strip_suffix('\n')
        .filter(|line| !line.contains('\n'))
        .ok_or_else(|| unexpected("not one line"))?;
    let mut values = line
        .split(' ')
        .filter_map(|word| word.strip_prefix(RESET_TYPE));
    let value = match (values.next(), values.next()) {
        (Some(value), None) if reset_type(value) => value,
        (None, _) => return Err(unexpected("no normal_reset_type")),
        (Some(value), None) => return Err(unexpected(&format!("reset type {value:?}"))),
        (Some(_), Some(_)) => return Err(unexpected("normal_reset_type twice")),
    };
    Ok(match value {
        "BR_POWEROFF_ALARM" => Boot::Alarm,
        "BR_PRESS_1S" => Boot::Press,
        "BR_POWEROFF_CHARGE" => Boot::Charger,
        _ => Boot::Other,
    })
}

#[must_use]
pub fn timer_target(on_at: UnixSeconds) -> UnixSeconds {
    UnixSeconds(on_at.0.saturating_add(KERNEL_LEAD_SECONDS))
}

#[derive(Debug)]
pub struct Cmdline(String);

impl Cmdline {
    pub fn open() -> Result<Self, HardwareError> {
        Self::at(Path::new(CMDLINE))
    }

    pub fn at(path: &Path) -> Result<Self, HardwareError> {
        fs::read_to_string(path)
            .map(Self)
            .map_err(|error| failed(CMDLINE_DEVICE, &error))
    }
}

impl BootReason for Cmdline {
    fn read(&self) -> Result<Boot, HardwareError> {
        reason(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const ALARM: &str = include_str!("../testdata/cmdline-alarm");
    const PRESS: &str = include_str!("../testdata/cmdline-press");
    const CHARGER: &str = include_str!("../testdata/cmdline-charger");
    const PANIC: &str = include_str!("../testdata/cmdline-panic");

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!("power-{}-{test}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_recorded_command_lines_name_why_the_phone_booted() -> Result<(), HardwareError> {
        assert_eq!(reason(ALARM)?, Boot::Alarm);
        assert_eq!(reason(PRESS)?, Boot::Press);
        assert_eq!(reason(CHARGER)?, Boot::Charger);
        assert_eq!(reason(PANIC)?, Boot::Other);
        Ok(())
    }

    #[test]
    fn a_line_without_one_well_formed_reset_type_is_a_hardware_error() {
        for text in [
            "loglevel=4 init=/init\n",
            "normal_reset_type=BR_POWEROFF_ALARM",
            "normal_reset_type=BR_POWEROFF_ALARM\n\n",
            "normal_reset_type=\n",
            "normal_reset_type=br_press_1s\n",
            "normal_reset_type=BR_PRESS_1S normal_reset_type=BR_POWEROFF_ALARM\n",
            "xnormal_reset_type=BR_POWEROFF_ALARM\n",
        ] {
            assert!(
                matches!(
                    reason(text),
                    Err(HardwareError::Failed {
                        device: CMDLINE_DEVICE,
                        ..
                    })
                ),
                "{text:?}"
            );
        }
    }

    #[test]
    fn the_kernel_arms_the_pmu_a_minute_before_the_timer() {
        assert_eq!(
            timer_target(UnixSeconds(1_794_562_576)),
            UnixSeconds(1_794_562_636)
        );
    }

    #[test]
    fn the_command_line_is_read_at_open() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("cmdline")?;
        let path = scratch.0.join("cmdline");
        fs::write(&path, ALARM)?;
        let cmdline = Cmdline::at(&path)?;
        fs::write(&path, PRESS)?;
        assert_eq!(cmdline.read()?, Boot::Alarm);
        assert_eq!(
            Cmdline::at(&scratch.0.join("absent")).err(),
            Some(HardwareError::Absent(CMDLINE_DEVICE))
        );
        Ok(())
    }
}
