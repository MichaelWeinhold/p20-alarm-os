use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use alarm_core::hardware::{HardwareError, Indicator};

pub const RED: &str = "/sys/class/leds/red";

const DEVICE: &str = "leds/red";
const TIMER: &str = "timer";
const LOWEST: &str = "1";
const OFF: &str = "0";
const DARK_MILLISECONDS: &str = "2000";

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

#[derive(Debug)]
pub struct RedLed {
    trigger: Node,
    brightness: Node,
    delay_off: PathBuf,
}

impl RedLed {
    pub fn open() -> Result<Self, HardwareError> {
        Self::at(Path::new(RED))
    }

    pub fn at(directory: &Path) -> Result<Self, HardwareError> {
        Ok(Self {
            trigger: Node::at(directory.join("trigger"))?,
            brightness: Node::at(directory.join("brightness"))?,
            delay_off: directory.join("delay_off"),
        })
    }
}

impl Indicator for RedLed {
    fn warn(&mut self) -> Result<(), HardwareError> {
        self.trigger.write(TIMER)?;
        self.brightness.write(LOWEST)?;
        Node::at(self.delay_off.clone())?.write(DARK_MILLISECONDS)
    }

    fn clear(&mut self) -> Result<(), HardwareError> {
        self.brightness.write(OFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> io::Result<Self> {
            let path =
                std::env::temp_dir().join(format!("indicator-{}-{test}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        fn led(test: &str) -> io::Result<Self> {
            let scratch = Self::new(test)?;
            scratch.file("trigger", "[none] timer\n")?;
            scratch.file("brightness", "0\n")?;
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
    fn a_warning_selects_the_timer_lights_the_lowest_current_and_darkens_for_two_seconds()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::led("warn")?;
        let mut led = RedLed::at(&scratch.0)?;
        scratch.file("delay_off", "0\n")?;
        led.warn()?;
        assert_eq!(scratch.read("trigger")?, TIMER);
        assert_eq!(scratch.read("brightness")?, LOWEST);
        assert_eq!(scratch.read("delay_off")?, DARK_MILLISECONDS);
        Ok(())
    }

    #[test]
    fn clearing_turns_the_led_off() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::led("clear")?;
        let mut led = RedLed::at(&scratch.0)?;
        scratch.file("delay_off", "0\n")?;
        led.warn()?;
        led.clear()?;
        assert_eq!(scratch.read("brightness")?, OFF);
        Ok(())
    }

    #[test]
    fn a_trigger_that_makes_no_delay_off_is_absent() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::led("no-delay")?;
        let mut led = RedLed::at(&scratch.0)?;
        assert_eq!(led.warn(), Err(HardwareError::Absent(DEVICE)));
        assert!(!scratch.0.join("delay_off").exists());
        Ok(())
    }

    #[test]
    fn an_absent_led_does_not_open() -> io::Result<()> {
        let scratch = Scratch::new("absent")?;
        assert_eq!(
            RedLed::at(&scratch.0).err(),
            Some(HardwareError::Absent(DEVICE))
        );
        scratch.file("trigger", "[none] timer\n")?;
        assert_eq!(
            RedLed::at(&scratch.0).err(),
            Some(HardwareError::Absent(DEVICE))
        );
        Ok(())
    }

    #[test]
    fn a_failed_write_is_a_failure() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::led("unwritable")?;
        let mut led = RedLed::at(&scratch.0)?;
        fs::remove_file(scratch.0.join("brightness"))?;
        fs::create_dir(scratch.0.join("brightness"))?;
        assert!(matches!(
            led.clear(),
            Err(HardwareError::Failed { device: DEVICE, .. })
        ));
        assert!(led.warn().is_err());
        Ok(())
    }
}
