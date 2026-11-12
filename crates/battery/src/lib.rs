use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use alarm_core::battery::{BatteryReading, ChargeStatus, Health, Percent};
use alarm_core::hardware::{Battery, HardwareError};

pub const BATTERY: &str = "/sys/class/power_supply/Battery";
pub const CHARGER: &str = "/sys/class/hw_power/charger/charge_data";

const CAPACITY: &str = "Battery/capacity";
const STATUS: &str = "Battery/status";
const VOLTAGE: &str = "Battery/voltage_now";
const TEMPERATURE: &str = "Battery/temp";
const CURRENT: &str = "Battery/current_now";
const HEALTH: &str = "Battery/health";
const ENABLE_CHARGER: &str = "charge_data/enable_charger";

#[derive(Debug)]
struct Node {
    device: &'static str,
    path: PathBuf,
}

impl Node {
    fn at(directory: &Path, device: &'static str) -> Result<Self, HardwareError> {
        let file = device.rsplit('/').next().unwrap_or(device);
        let path = directory.join(file);
        if path.is_file() {
            Ok(Self { device, path })
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

#[derive(Debug)]
pub struct SysfsBattery {
    capacity: Node,
    status: Node,
    voltage: Node,
    temperature: Node,
    current: Node,
    health: Node,
    enable_charger: Node,
}

impl SysfsBattery {
    pub fn open() -> Result<Self, HardwareError> {
        Self::at(Path::new(BATTERY), Path::new(CHARGER))
    }

    pub fn at(battery: &Path, charger: &Path) -> Result<Self, HardwareError> {
        Ok(Self {
            capacity: Node::at(battery, CAPACITY)?,
            status: Node::at(battery, STATUS)?,
            voltage: Node::at(battery, VOLTAGE)?,
            temperature: Node::at(battery, TEMPERATURE)?,
            current: Node::at(battery, CURRENT)?,
            health: Node::at(battery, HEALTH)?,
            enable_charger: Node::at(charger, ENABLE_CHARGER)?,
        })
    }
}

impl Battery for SysfsBattery {
    fn read(&self) -> Result<BatteryReading, HardwareError> {
        Ok(BatteryReading {
            capacity: capacity(&self.capacity.read()?)?,
            status: status(&self.status.read()?)?,
            microvolts: number(VOLTAGE, &self.voltage.read()?)?,
            decicelsius: number(TEMPERATURE, &self.temperature.read()?)?,
            milliamps: number(CURRENT, &self.current.read()?)?,
            health: health(&self.health.read()?)?,
        })
    }

    fn charger_enabled(&self) -> Result<bool, HardwareError> {
        enabled(&self.enable_charger.read()?)
    }

    fn set_charger_enabled(&mut self, enabled: bool) -> Result<(), HardwareError> {
        self.enable_charger.write(if enabled { "1" } else { "0" })
    }
}

fn unexpected(device: &'static str, text: &str) -> HardwareError {
    HardwareError::Failed {
        device,
        reason: format!("unexpected {text:?}"),
    }
}

fn line<'a>(device: &'static str, text: &'a str) -> Result<&'a str, HardwareError> {
    text.strip_suffix('\n')
        .filter(|value| !value.is_empty() && !value.contains('\n'))
        .ok_or_else(|| unexpected(device, text))
}

fn number<T: FromStr>(device: &'static str, text: &str) -> Result<T, HardwareError> {
    line(device, text)?
        .parse()
        .map_err(|_| unexpected(device, text))
}

fn capacity(text: &str) -> Result<Percent, HardwareError> {
    Percent::new(number(CAPACITY, text)?).map_err(|_| unexpected(CAPACITY, text))
}

fn status(text: &str) -> Result<ChargeStatus, HardwareError> {
    match line(STATUS, text)? {
        "Charging" => Ok(ChargeStatus::Charging),
        "Full" => Ok(ChargeStatus::Full),
        "Not charging" => Ok(ChargeStatus::NotCharging),
        "Discharging" => Ok(ChargeStatus::Discharging),
        _ => Err(unexpected(STATUS, text)),
    }
}

fn health(text: &str) -> Result<Health, HardwareError> {
    match line(HEALTH, text)? {
        "Unknown" => Ok(Health::Unknown),
        "Good" => Ok(Health::Good),
        "Overheat" => Ok(Health::Overheat),
        "Dead" => Ok(Health::Dead),
        "Over voltage" => Ok(Health::OverVoltage),
        "Unspecified failure" => Ok(Health::UnspecifiedFailure),
        "Cold" => Ok(Health::Cold),
        "Watchdog timer expire" => Ok(Health::WatchdogTimerExpire),
        "Safety timer expire" => Ok(Health::SafetyTimerExpire),
        "Over current" => Ok(Health::OverCurrent),
        "Warm" => Ok(Health::Warm),
        "Cool" => Ok(Health::Cool),
        "Hot" => Ok(Health::Hot),
        _ => Err(unexpected(HEALTH, text)),
    }
}

fn enabled(text: &str) -> Result<bool, HardwareError> {
    match line(ENABLE_CHARGER, text)? {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(unexpected(ENABLE_CHARGER, text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORDED: [(&str, &str); 10] = [
        ("capacity", include_str!("../testdata/capacity")),
        ("status-full", include_str!("../testdata/status-full")),
        (
            "status-not-charging",
            include_str!("../testdata/status-not-charging"),
        ),
        ("voltage_now", include_str!("../testdata/voltage_now")),
        ("temp", include_str!("../testdata/temp")),
        ("current_now", include_str!("../testdata/current_now")),
        (
            "current_now-from-battery",
            include_str!("../testdata/current_now-from-battery"),
        ),
        ("health", include_str!("../testdata/health")),
        (
            "enable_charger-on",
            include_str!("../testdata/enable_charger-on"),
        ),
        (
            "enable_charger-off",
            include_str!("../testdata/enable_charger-off"),
        ),
    ];

    fn recorded(name: &str) -> &'static str {
        RECORDED
            .iter()
            .find(|(fixture, _)| *fixture == name)
            .map_or("", |(_, text)| text)
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!("battery-{}-{test}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        fn with(&self, file: &str, fixture: &str) -> io::Result<&Self> {
            fs::write(self.0.join(file), recorded(fixture))?;
            Ok(self)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn recorded_battery(test: &str) -> io::Result<Scratch> {
        let scratch = Scratch::new(test)?;
        scratch
            .with("capacity", "capacity")?
            .with("status", "status-full")?
            .with("voltage_now", "voltage_now")?
            .with("temp", "temp")?
            .with("current_now", "current_now-from-battery")?
            .with("health", "health")?
            .with("enable_charger", "enable_charger-on")?;
        Ok(scratch)
    }

    #[test]
    fn the_recorded_nodes_parse_into_typed_values() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(capacity(recorded("capacity"))?, Percent::new(100)?);
        assert_eq!(status(recorded("status-full"))?, ChargeStatus::Full);
        assert_eq!(
            status(recorded("status-not-charging"))?,
            ChargeStatus::NotCharging
        );
        assert_eq!(number::<u32>(VOLTAGE, recorded("voltage_now"))?, 4_339_811);
        assert_eq!(number::<i32>(TEMPERATURE, recorded("temp"))?, 250);
        assert_eq!(number::<i32>(CURRENT, recorded("current_now"))?, 0);
        assert_eq!(
            number::<i32>(CURRENT, recorded("current_now-from-battery"))?,
            -34
        );
        assert_eq!(health(recorded("health"))?, Health::Good);
        assert!(enabled(recorded("enable_charger-on"))?);
        assert!(!enabled(recorded("enable_charger-off"))?);
        Ok(())
    }

    #[test]
    fn a_battery_off_the_cable_reads_discharging() -> Result<(), HardwareError> {
        assert_eq!(status("Discharging\n")?, ChargeStatus::Discharging);
        Ok(())
    }

    #[test]
    fn malformed_bytes_are_a_hardware_error() {
        assert_eq!(capacity("101\n"), Err(unexpected(CAPACITY, "101\n")));
        assert_eq!(capacity("100"), Err(unexpected(CAPACITY, "100")));
        assert_eq!(capacity("\n"), Err(unexpected(CAPACITY, "\n")));
        assert_eq!(capacity("1 00\n"), Err(unexpected(CAPACITY, "1 00\n")));
        assert_eq!(status("Unknown\n"), Err(unexpected(STATUS, "Unknown\n")));
        assert_eq!(
            status("Full\nFull\n"),
            Err(unexpected(STATUS, "Full\nFull\n"))
        );
        assert_eq!(health("Swollen\n"), Err(unexpected(HEALTH, "Swollen\n")));
        assert_eq!(
            number::<u32>(VOLTAGE, "-1\n"),
            Err(unexpected(VOLTAGE, "-1\n"))
        );
        assert_eq!(enabled("2\n"), Err(unexpected(ENABLE_CHARGER, "2\n")));
    }

    #[test]
    fn the_adapter_reads_every_node_and_writes_only_enable_charger()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = recorded_battery("reads")?;
        let mut battery = SysfsBattery::at(&scratch.0, &scratch.0)?;
        let reading = battery.read()?;
        assert_eq!(reading.capacity, Percent::new(100)?);
        assert_eq!(reading.status, ChargeStatus::Full);
        assert_eq!(reading.milliamps, -34);
        assert!(battery.charger_enabled()?);
        battery.set_charger_enabled(false)?;
        assert_eq!(fs::read_to_string(scratch.0.join("enable_charger"))?, "0");
        for (file, fixture) in [
            ("capacity", "capacity"),
            ("status", "status-full"),
            ("health", "health"),
        ] {
            assert_eq!(fs::read_to_string(scratch.0.join(file))?, recorded(fixture));
        }
        Ok(())
    }

    #[test]
    fn an_absent_node_does_not_open() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = recorded_battery("absent")?;
        fs::remove_file(scratch.0.join("health"))?;
        assert_eq!(
            SysfsBattery::at(&scratch.0, &scratch.0).err(),
            Some(HardwareError::Absent(HEALTH))
        );
        Ok(())
    }

    #[test]
    fn a_node_gone_after_opening_is_absent() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = recorded_battery("gone")?;
        let battery = SysfsBattery::at(&scratch.0, &scratch.0)?;
        fs::remove_file(scratch.0.join("temp"))?;
        assert_eq!(battery.read(), Err(HardwareError::Absent(TEMPERATURE)));
        Ok(())
    }

    #[test]
    fn a_malformed_node_fails_the_whole_reading() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = recorded_battery("malformed")?;
        let battery = SysfsBattery::at(&scratch.0, &scratch.0)?;
        fs::write(scratch.0.join("status"), "Unknown\n")?;
        assert_eq!(battery.read(), Err(unexpected(STATUS, "Unknown\n")));
        Ok(())
    }
}
