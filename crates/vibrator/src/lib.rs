use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use alarm_core::hardware::{HardwareError, Vibrator};

pub const VIBRATOR: &str = "/sys/class/leds/vibrator";

const DEVICE: &str = "leds/vibrator";
const TRANSIENT: &str = "transient";
const ON: &str = "1";
const OFF: &str = "0";

#[derive(Debug)]
struct Node(PathBuf);

impl Node {
    fn at(path: PathBuf) -> Result<Self, HardwareError> {
        if path.is_file() {
            Ok(Self(path))
        } else {
            Err(HardwareError::Absent(DEVICE))
        }
    }

    fn read(&self) -> Result<String, HardwareError> {
        fs::read_to_string(&self.0).map_err(|error| failed(&error))
    }

    fn write(&self, text: &str) -> Result<(), HardwareError> {
        fs::write(&self.0, text).map_err(|error| failed(&error))
    }
}

fn failed(error: &io::Error) -> HardwareError {
    if error.kind() == io::ErrorKind::NotFound {
        HardwareError::Absent(DEVICE)
    } else {
        HardwareError::Failed {
            device: DEVICE,
            reason: error.to_string(),
        }
    }
}

fn unexpected(text: &str) -> HardwareError {
    HardwareError::Failed {
        device: DEVICE,
        reason: format!("unexpected trigger list {text:?}"),
    }
}

fn offers_transient(text: &str) -> Result<bool, HardwareError> {
    let words = text
        .strip_suffix('\n')
        .filter(|line| !line.is_empty() && !line.contains('\n'))
        .ok_or_else(|| unexpected(text))?;
    Ok(words
        .split(' ')
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.strip_prefix('[')
                .and_then(|word| word.strip_suffix(']'))
                .unwrap_or(word)
        })
        .any(|word| word == TRANSIENT))
}

#[derive(Debug)]
pub struct LedVibrator {
    duration: Node,
    state: Node,
    activate: Node,
}

impl LedVibrator {
    pub fn open() -> Result<Self, HardwareError> {
        Self::at(Path::new(VIBRATOR))
    }

    pub fn at(directory: &Path) -> Result<Self, HardwareError> {
        let trigger = Node::at(directory.join("trigger"))?;
        let offered = trigger.read()?;
        if !offers_transient(&offered)? {
            return Err(HardwareError::Failed {
                device: DEVICE,
                reason: format!("no {TRANSIENT} trigger in {offered:?}"),
            });
        }
        trigger.write(TRANSIENT)?;
        Ok(Self {
            duration: Node::at(directory.join("duration"))?,
            state: Node::at(directory.join("state"))?,
            activate: Node::at(directory.join("activate"))?,
        })
    }
}

impl Vibrator for LedVibrator {
    fn vibrate(&mut self, milliseconds: u32) -> Result<(), HardwareError> {
        self.duration.write(&milliseconds.to_string())?;
        self.state.write(ON)?;
        self.activate.write(ON)
    }

    fn stop(&mut self) -> Result<(), HardwareError> {
        self.activate.write(OFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIGGER: &str = include_str!("../testdata/trigger");
    const ACTIVATE: &str = include_str!("../testdata/activate");

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!("vibrator-{}-{test}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        fn recorded(test: &str) -> io::Result<Self> {
            let scratch = Self::new(test)?;
            scratch.file("trigger", TRIGGER)?;
            for name in ["activate", "duration", "state"] {
                scratch.file(name, ACTIVATE)?;
            }
            Ok(scratch)
        }

        fn file(&self, name: &str, text: &str) -> io::Result<()> {
            fs::write(self.0.join(name), text)
        }

        fn read(&self, name: &str) -> io::Result<String> {
            fs::read_to_string(self.0.join(name))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_recorded_trigger_list_offers_transient() -> Result<(), HardwareError> {
        assert!(offers_transient(TRIGGER)?);
        assert!(offers_transient("none timer transient\n")?);
        assert!(!offers_transient("none [timer] mmc0 \n")?);
        assert!(!offers_transient("none [transient-ish]\n")?);
        Ok(())
    }

    #[test]
    fn a_malformed_trigger_list_is_a_hardware_error() {
        for text in ["", "\n", "none [transient]", "none\n[transient]\n"] {
            assert_eq!(offers_transient(text), Err(unexpected(text)));
        }
    }

    #[test]
    fn opening_selects_the_transient_trigger() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::recorded("open")?;
        LedVibrator::at(&scratch.0)?;
        assert_eq!(scratch.read("trigger")?, TRANSIENT);
        assert_eq!(scratch.read("activate")?, ACTIVATE);
        Ok(())
    }

    #[test]
    fn a_pulse_writes_its_duration_before_activating() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::recorded("pulse")?;
        let mut vibrator = LedVibrator::at(&scratch.0)?;
        vibrator.vibrate(1_000)?;
        assert_eq!(scratch.read("duration")?, "1000");
        assert_eq!(scratch.read("state")?, ON);
        assert_eq!(scratch.read("activate")?, ON);
        vibrator.stop()?;
        assert_eq!(scratch.read("activate")?, OFF);
        assert_eq!(scratch.read("duration")?, "1000");
        Ok(())
    }

    #[test]
    fn an_absent_vibrator_does_not_open() -> io::Result<()> {
        let scratch = Scratch::new("absent")?;
        assert_eq!(
            LedVibrator::at(&scratch.0).err(),
            Some(HardwareError::Absent(DEVICE))
        );
        scratch.file("trigger", TRIGGER)?;
        assert_eq!(
            LedVibrator::at(&scratch.0).err(),
            Some(HardwareError::Absent(DEVICE))
        );
        Ok(())
    }

    #[test]
    fn a_kernel_without_the_transient_trigger_does_not_open() -> io::Result<()> {
        let scratch = Scratch::recorded("no-transient")?;
        scratch.file("trigger", "none [timer]\n")?;
        assert!(matches!(
            LedVibrator::at(&scratch.0),
            Err(HardwareError::Failed { device: DEVICE, .. })
        ));
        assert_eq!(scratch.read("trigger")?, "none [timer]\n");
        Ok(())
    }

    #[test]
    fn a_failed_write_is_a_failure() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::recorded("unwritable")?;
        let mut vibrator = LedVibrator::at(&scratch.0)?;
        fs::remove_file(scratch.0.join("activate"))?;
        fs::create_dir(scratch.0.join("activate"))?;
        assert!(matches!(
            vibrator.vibrate(1_000),
            Err(HardwareError::Failed { device: DEVICE, .. })
        ));
        assert!(vibrator.stop().is_err());
        Ok(())
    }
}
