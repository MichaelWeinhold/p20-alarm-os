use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use alarm_core::hardware::{HardwareError, Suspend, WakeAlarm};
use alarm_core::time::UnixSeconds;

pub const WAKEALARM: &str = "/sys/class/rtc/rtc0/wakealarm";
pub const STATE: &str = "/sys/power/state";

const WAKE_DEVICE: &str = "rtc0/wakealarm";
const STATE_DEVICE: &str = "power/state";
const DISARM: &str = "0";
const MEM: &str = "mem";

#[derive(Debug)]
struct Node {
    device: &'static str,
    path: PathBuf,
}

impl Node {
    fn at(path: &Path, device: &'static str) -> Result<Self, HardwareError> {
        if path.is_file() {
            Ok(Self {
                device,
                path: path.to_path_buf(),
            })
        } else {
            Err(HardwareError::Absent(device))
        }
    }

    fn read(&self) -> Result<String, HardwareError> {
        fs::read_to_string(&self.path).map_err(|error| self.failed(&error))
    }

    fn write(&self, text: &str) -> Result<(), HardwareError> {
        fs::write(&self.path, text).map_err(|error| self.failed(&error))
    }

    fn failed(&self, error: &io::Error) -> HardwareError {
        if error.kind() == io::ErrorKind::NotFound {
            HardwareError::Absent(self.device)
        } else {
            HardwareError::Failed {
                device: self.device,
                reason: error.to_string(),
            }
        }
    }
}

fn unexpected(text: &str) -> HardwareError {
    HardwareError::Failed {
        device: WAKE_DEVICE,
        reason: format!("unexpected {text:?}"),
    }
}

fn armed(text: &str) -> Result<Option<UnixSeconds>, HardwareError> {
    if text.is_empty() {
        return Ok(None);
    }
    text.strip_suffix('\n')
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse().ok())
        .map(|seconds| Some(UnixSeconds(seconds)))
        .ok_or_else(|| unexpected(text))
}

fn confirmed(expected: Option<UnixSeconds>, text: &str) -> Result<(), HardwareError> {
    let read = armed(text)?;
    if read == expected {
        Ok(())
    } else {
        Err(HardwareError::Failed {
            device: WAKE_DEVICE,
            reason: format!("armed for {read:?}, not {expected:?}"),
        })
    }
}

#[derive(Debug)]
pub struct SysfsWakeAlarm(Node);

impl SysfsWakeAlarm {
    pub fn open() -> Result<Self, HardwareError> {
        Self::at(Path::new(WAKEALARM))
    }

    pub fn at(path: &Path) -> Result<Self, HardwareError> {
        Node::at(path, WAKE_DEVICE).map(Self)
    }
}

impl WakeAlarm for SysfsWakeAlarm {
    fn arm(&mut self, at: UnixSeconds) -> Result<(), HardwareError> {
        self.0.write(DISARM)?;
        self.0.write(&at.0.to_string())?;
        confirmed(Some(at), &self.0.read()?)
    }

    fn disarm(&mut self) -> Result<(), HardwareError> {
        self.0.write(DISARM)?;
        confirmed(None, &self.0.read()?)
    }
}

#[derive(Debug)]
pub struct SysfsSuspend(Node);

impl SysfsSuspend {
    pub fn open() -> Result<Self, HardwareError> {
        Self::at(Path::new(STATE))
    }

    pub fn at(path: &Path) -> Result<Self, HardwareError> {
        Node::at(path, STATE_DEVICE).map(Self)
    }
}

impl Suspend for SysfsSuspend {
    fn suspend(&mut self) -> Result<(), HardwareError> {
        self.0.write(MEM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARMED: &str = include_str!("../testdata/wakealarm-armed");
    const DISARMED: &str = include_str!("../testdata/wakealarm-disarmed");
    const RECORDED_AT: UnixSeconds = UnixSeconds(1_794_914_885);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!("suspend-{}-{test}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        fn file(&self, name: &str, text: &str) -> io::Result<PathBuf> {
            let path = self.0.join(name);
            fs::write(&path, text)?;
            Ok(path)
        }

        fn unwritable(path: &Path) -> io::Result<()> {
            fs::remove_file(path)?;
            fs::create_dir(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_recorded_reads_parse_into_the_armed_instant() -> Result<(), HardwareError> {
        assert_eq!(armed(ARMED)?, Some(RECORDED_AT));
        assert_eq!(armed(DISARMED)?, None);
        confirmed(Some(RECORDED_AT), ARMED)?;
        confirmed(None, DISARMED)
    }

    #[test]
    fn a_read_back_other_than_the_one_expected_is_a_failure() {
        assert!(matches!(
            confirmed(Some(UnixSeconds(RECORDED_AT.0 + 1)), ARMED),
            Err(HardwareError::Failed {
                device: WAKE_DEVICE,
                ..
            })
        ));
        assert!(confirmed(None, ARMED).is_err());
        assert!(confirmed(Some(RECORDED_AT), DISARMED).is_err());
    }

    #[test]
    fn malformed_bytes_are_a_hardware_error() {
        for text in [
            "1794914885",
            "\n",
            "0x6afc\n",
            "-1\n",
            "1794914885\n1\n",
            " 1\n",
        ] {
            assert_eq!(armed(text), Err(unexpected(text)));
        }
    }

    #[test]
    fn arming_writes_the_epoch_last_and_checks_what_reads_back()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("arm")?;
        let path = scratch.file("wakealarm", DISARMED)?;
        let mut wake = SysfsWakeAlarm::at(&path)?;
        let error = wake.arm(RECORDED_AT).err();
        assert_eq!(fs::read_to_string(&path)?, "1794914885");
        assert_eq!(error, Some(unexpected("1794914885")));
        Ok(())
    }

    #[test]
    fn disarming_writes_zero_and_refuses_anything_read_back()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("disarm")?;
        let path = scratch.file("wakealarm", ARMED)?;
        let mut wake = SysfsWakeAlarm::at(&path)?;
        let error = wake.disarm().err();
        assert_eq!(fs::read_to_string(&path)?, DISARM);
        assert_eq!(error, Some(unexpected(DISARM)));
        Ok(())
    }

    #[test]
    fn a_failed_write_is_a_failure_not_an_armed_alarm() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("unwritable")?;
        let path = scratch.file("wakealarm", DISARMED)?;
        let mut wake = SysfsWakeAlarm::at(&path)?;
        Scratch::unwritable(&path)?;
        assert!(matches!(
            wake.arm(RECORDED_AT),
            Err(HardwareError::Failed {
                device: WAKE_DEVICE,
                ..
            })
        ));
        assert!(wake.disarm().is_err());
        Ok(())
    }

    #[test]
    fn an_absent_node_does_not_open() -> io::Result<()> {
        let scratch = Scratch::new("absent")?;
        assert_eq!(
            SysfsWakeAlarm::at(&scratch.0.join("wakealarm")).err(),
            Some(HardwareError::Absent(WAKE_DEVICE))
        );
        assert_eq!(
            SysfsSuspend::at(&scratch.0.join("state")).err(),
            Some(HardwareError::Absent(STATE_DEVICE))
        );
        Ok(())
    }

    #[test]
    fn suspending_writes_mem() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("mem")?;
        let path = scratch.file("state", "freeze mem\n")?;
        SysfsSuspend::at(&path)?.suspend()?;
        assert_eq!(fs::read_to_string(&path)?, MEM);
        Ok(())
    }

    #[test]
    fn a_suspend_the_kernel_aborts_is_a_failure() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("aborted")?;
        let path = scratch.file("state", "freeze mem\n")?;
        let mut suspend = SysfsSuspend::at(&path)?;
        Scratch::unwritable(&path)?;
        assert!(matches!(
            suspend.suspend(),
            Err(HardwareError::Failed {
                device: STATE_DEVICE,
                ..
            })
        ));
        Ok(())
    }
}
