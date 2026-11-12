use std::fmt;

use crate::hardware::{Battery, HardwareError};

const LOWEST_LIMIT: u8 = 50;
const FULL: u8 = 100;

pub const LOW_MARK: Percent = Percent(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryError {
    Percent(u8),
    Limit(u8),
}

impl fmt::Display for BatteryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Percent(value) => write!(formatter, "{value} % is not in 0..={FULL}"),
            Self::Limit(value) => {
                write!(
                    formatter,
                    "a charge limit of {value} % is not in {LOWEST_LIMIT}..={FULL}"
                )
            }
        }
    }
}

impl std::error::Error for BatteryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Percent(u8);

impl Percent {
    pub fn new(value: u8) -> Result<Self, BatteryError> {
        if value > FULL {
            return Err(BatteryError::Percent(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChargeStatus {
    Charging,
    Full,
    NotCharging,
    Discharging,
}

impl ChargeStatus {
    #[must_use]
    pub fn cable_in(self) -> bool {
        !matches!(self, Self::Discharging)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Health {
    Unknown,
    Good,
    Overheat,
    Dead,
    OverVoltage,
    UnspecifiedFailure,
    Cold,
    WatchdogTimerExpire,
    SafetyTimerExpire,
    OverCurrent,
    Warm,
    Cool,
    Hot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BatteryReading {
    pub capacity: Percent,
    pub status: ChargeStatus,
    pub microvolts: u32,
    pub decicelsius: i32,
    pub milliamps: i32,
    pub health: Health,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChargeLimit(Percent);

impl ChargeLimit {
    pub const DEFAULT: Self = Self(Percent(80));

    pub fn new(value: u8) -> Result<Self, BatteryError> {
        if !(LOWEST_LIMIT..=FULL).contains(&value) {
            return Err(BatteryError::Limit(value));
        }
        Ok(Self(Percent(value)))
    }

    #[must_use]
    pub fn percent(self) -> Percent {
        self.0
    }
}

impl Default for ChargeLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChargeStep {
    Stopped,
    Left,
}

#[must_use]
pub fn shows_low_mark(capacity: Percent) -> bool {
    capacity <= LOW_MARK
}

#[must_use]
pub fn wants_mark(battery: &impl Battery) -> bool {
    battery
        .read()
        .map_or(true, |reading| shows_low_mark(reading.capacity))
}

pub fn stop_at_limit(
    limit: ChargeLimit,
    battery: &mut impl Battery,
) -> Result<ChargeStep, HardwareError> {
    let enabled = battery.charger_enabled()?;
    let reading = match battery.read() {
        Ok(reading) => reading,
        Err(error) => {
            if !enabled {
                battery.set_charger_enabled(true)?;
            }
            return Err(error);
        }
    };
    if enabled && reading.status.cable_in() && reading.capacity >= limit.percent() {
        battery.set_charger_enabled(false)?;
        return Ok(ChargeStep::Stopped);
    }
    Ok(ChargeStep::Left)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBattery {
        reading: Option<BatteryReading>,
        enabled: Option<bool>,
        writes: Vec<bool>,
    }

    impl FakeBattery {
        fn new(capacity: u8, status: ChargeStatus, enabled: bool) -> Result<Self, BatteryError> {
            Ok(Self {
                reading: Some(BatteryReading {
                    capacity: Percent::new(capacity)?,
                    status,
                    microvolts: 4_339_811,
                    decicelsius: 250,
                    milliamps: 120,
                    health: Health::Good,
                }),
                enabled: Some(enabled),
                writes: Vec::new(),
            })
        }

        fn unreadable(enabled: Option<bool>) -> Self {
            Self {
                reading: None,
                enabled,
                writes: Vec::new(),
            }
        }

        fn plug_in(&mut self) {
            self.enabled = Some(true);
            if let Some(reading) = &mut self.reading {
                reading.status = ChargeStatus::Charging;
            }
        }
    }

    impl Battery for FakeBattery {
        fn read(&self) -> Result<BatteryReading, HardwareError> {
            self.reading
                .ok_or(HardwareError::Absent("Battery/capacity"))
        }

        fn charger_enabled(&self) -> Result<bool, HardwareError> {
            self.enabled
                .ok_or(HardwareError::Absent("charge_data/enable_charger"))
        }

        fn set_charger_enabled(&mut self, enabled: bool) -> Result<(), HardwareError> {
            self.enabled = Some(enabled);
            self.writes.push(enabled);
            Ok(())
        }
    }

    #[test]
    fn one_under_the_limit_the_charge_goes_on() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = FakeBattery::new(79, ChargeStatus::Charging, true)?;
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery)?,
            ChargeStep::Left
        );
        assert_eq!(battery.writes, Vec::<bool>::new());
        Ok(())
    }

    #[test]
    fn at_the_limit_on_the_cable_the_charge_stops() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = FakeBattery::new(80, ChargeStatus::Charging, true)?;
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery)?,
            ChargeStep::Stopped
        );
        assert_eq!(battery.writes, [false]);
        Ok(())
    }

    #[test]
    fn a_limit_of_100_lets_the_charge_reach_full() -> Result<(), Box<dyn std::error::Error>> {
        let limit = ChargeLimit::new(100)?;
        let mut charging = FakeBattery::new(99, ChargeStatus::Charging, true)?;
        assert_eq!(stop_at_limit(limit, &mut charging)?, ChargeStep::Left);
        let mut full = FakeBattery::new(100, ChargeStatus::Full, true)?;
        assert_eq!(stop_at_limit(limit, &mut full)?, ChargeStep::Stopped);
        assert_eq!(full.writes, [false]);
        Ok(())
    }

    #[test]
    fn the_limit_is_80_by_default_and_runs_from_50_to_100() -> Result<(), BatteryError> {
        assert_eq!(ChargeLimit::default().percent(), Percent::new(80)?);
        assert_eq!(ChargeLimit::new(50)?.percent(), Percent::new(50)?);
        assert_eq!(ChargeLimit::new(49), Err(BatteryError::Limit(49)));
        assert_eq!(ChargeLimit::new(101), Err(BatteryError::Limit(101)));
        Ok(())
    }

    #[test]
    fn a_capacity_above_full_is_refused() {
        assert_eq!(Percent::new(101), Err(BatteryError::Percent(101)));
    }

    #[test]
    fn after_a_plug_in_resets_the_charger_the_charge_stops_again()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = FakeBattery::new(85, ChargeStatus::NotCharging, false)?;
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery)?,
            ChargeStep::Left
        );
        battery.plug_in();
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery)?,
            ChargeStep::Stopped
        );
        assert_eq!(battery.writes, [false]);
        assert_eq!(battery.enabled, Some(false));
        Ok(())
    }

    #[test]
    fn off_the_cable_nothing_is_written() -> Result<(), Box<dyn std::error::Error>> {
        for enabled in [true, false] {
            let mut battery = FakeBattery::new(95, ChargeStatus::Discharging, enabled)?;
            assert_eq!(
                stop_at_limit(ChargeLimit::DEFAULT, &mut battery)?,
                ChargeStep::Left
            );
            assert_eq!(battery.writes, Vec::<bool>::new());
        }
        Ok(())
    }

    #[test]
    fn below_the_limit_a_stopped_charge_stays_stopped() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = FakeBattery::new(70, ChargeStatus::NotCharging, false)?;
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery)?,
            ChargeStep::Left
        );
        assert_eq!(battery.writes, Vec::<bool>::new());
        Ok(())
    }

    #[test]
    fn the_low_mark_shows_at_20_and_below() -> Result<(), BatteryError> {
        assert!(shows_low_mark(Percent::new(0)?));
        assert!(shows_low_mark(Percent::new(20)?));
        assert!(!shows_low_mark(Percent::new(21)?));
        Ok(())
    }

    #[test]
    fn the_mark_follows_the_capacity() -> Result<(), BatteryError> {
        assert!(wants_mark(&FakeBattery::new(
            20,
            ChargeStatus::Discharging,
            true
        )?));
        assert!(!wants_mark(&FakeBattery::new(
            21,
            ChargeStatus::Discharging,
            true
        )?));
        Ok(())
    }

    #[test]
    fn an_unreadable_battery_shows_the_mark() {
        assert!(wants_mark(&FakeBattery::unreadable(Some(true))));
    }

    #[test]
    fn a_failing_battery_read_turns_a_stopped_charger_back_on() {
        let mut battery = FakeBattery::unreadable(Some(false));
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery),
            Err(HardwareError::Absent("Battery/capacity"))
        );
        assert_eq!(battery.writes, [true]);
    }

    #[test]
    fn a_failing_battery_read_leaves_a_running_charger_alone() {
        let mut battery = FakeBattery::unreadable(Some(true));
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery),
            Err(HardwareError::Absent("Battery/capacity"))
        );
        assert_eq!(battery.writes, Vec::<bool>::new());
    }

    #[test]
    fn an_unreadable_charger_is_never_written() {
        let mut battery = FakeBattery::unreadable(None);
        assert_eq!(
            stop_at_limit(ChargeLimit::DEFAULT, &mut battery),
            Err(HardwareError::Absent("charge_data/enable_charger"))
        );
        assert_eq!(battery.writes, Vec::<bool>::new());
    }
}
