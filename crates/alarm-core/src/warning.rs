use crate::battery::{BatteryReading, shows_low_mark};
use crate::hardware::{HardwareError, Indicator};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum State {
    #[default]
    Unset,
    Shown(bool),
    Failed(bool),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Warning {
    state: State,
}

#[must_use]
pub fn low_off_the_cable(reading: &BatteryReading) -> bool {
    shows_low_mark(reading.capacity) && !reading.status.cable_in()
}

impl Warning {
    #[must_use]
    pub fn unset() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn shown(&self) -> bool {
        self.state == State::Shown(true)
    }

    pub fn tick(
        &mut self,
        reading: Option<BatteryReading>,
        indicator: &mut impl Indicator,
    ) -> Result<(), HardwareError> {
        let wanted = reading.is_some_and(|reading| low_off_the_cable(&reading));
        if matches!(self.state, State::Shown(shown) | State::Failed(shown) if shown == wanted) {
            return Ok(());
        }
        let written = if wanted {
            indicator.warn()
        } else {
            indicator.clear()
        };
        self.state = if written.is_ok() {
            State::Shown(wanted)
        } else {
            State::Failed(wanted)
        };
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::{BatteryError, ChargeStatus, Health, Percent};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Warn,
        Clear,
    }

    #[derive(Default)]
    struct FakeIndicator {
        calls: Vec<Call>,
        failing: bool,
    }

    impl FakeIndicator {
        fn failing() -> Self {
            Self {
                calls: Vec::new(),
                failing: true,
            }
        }

        fn answer(&self) -> Result<(), HardwareError> {
            if self.failing {
                Err(HardwareError::Failed {
                    device: "indicator",
                    reason: "refused".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    impl Indicator for FakeIndicator {
        fn warn(&mut self) -> Result<(), HardwareError> {
            self.calls.push(Call::Warn);
            self.answer()
        }

        fn clear(&mut self) -> Result<(), HardwareError> {
            self.calls.push(Call::Clear);
            self.answer()
        }
    }

    fn reading(capacity: u8, status: ChargeStatus) -> Result<BatteryReading, BatteryError> {
        Ok(BatteryReading {
            capacity: Percent::new(capacity)?,
            status,
            microvolts: 3_700_000,
            decicelsius: 250,
            milliamps: -50,
            health: Health::Good,
        })
    }

    #[test]
    fn only_the_mark_and_below_off_the_cable_is_low() -> Result<(), BatteryError> {
        assert!(low_off_the_cable(&reading(20, ChargeStatus::Discharging)?));
        assert!(low_off_the_cable(&reading(0, ChargeStatus::Discharging)?));
        assert!(!low_off_the_cable(&reading(21, ChargeStatus::Discharging)?));
        for status in [
            ChargeStatus::Charging,
            ChargeStatus::Full,
            ChargeStatus::NotCharging,
        ] {
            assert!(!low_off_the_cable(&reading(20, status)?));
        }
        Ok(())
    }

    #[test]
    fn the_first_tick_clears_whatever_the_boot_left() -> Result<(), Box<dyn std::error::Error>> {
        let mut indicator = FakeIndicator::default();
        let mut warning = Warning::unset();
        warning.tick(
            Some(reading(50, ChargeStatus::Discharging)?),
            &mut indicator,
        )?;
        assert_eq!(indicator.calls, [Call::Clear]);
        assert!(!warning.shown());
        Ok(())
    }

    #[test]
    fn a_low_battery_off_the_cable_warns_once() -> Result<(), Box<dyn std::error::Error>> {
        let mut indicator = FakeIndicator::default();
        let mut warning = Warning::unset();
        for capacity in [20, 19, 18] {
            warning.tick(
                Some(reading(capacity, ChargeStatus::Discharging)?),
                &mut indicator,
            )?;
        }
        assert_eq!(indicator.calls, [Call::Warn]);
        assert!(warning.shown());
        Ok(())
    }

    #[test]
    fn the_cable_clears_it_and_pulling_it_warns_again() -> Result<(), Box<dyn std::error::Error>> {
        let mut indicator = FakeIndicator::default();
        let mut warning = Warning::unset();
        for status in [
            ChargeStatus::Discharging,
            ChargeStatus::Charging,
            ChargeStatus::Charging,
            ChargeStatus::Discharging,
        ] {
            warning.tick(Some(reading(15, status)?), &mut indicator)?;
        }
        assert_eq!(indicator.calls, [Call::Warn, Call::Clear, Call::Warn]);
        Ok(())
    }

    #[test]
    fn a_charge_above_the_mark_clears_it() -> Result<(), Box<dyn std::error::Error>> {
        let mut indicator = FakeIndicator::default();
        let mut warning = Warning::unset();
        warning.tick(
            Some(reading(20, ChargeStatus::Discharging)?),
            &mut indicator,
        )?;
        warning.tick(
            Some(reading(21, ChargeStatus::Discharging)?),
            &mut indicator,
        )?;
        assert_eq!(indicator.calls, [Call::Warn, Call::Clear]);
        Ok(())
    }

    #[test]
    fn an_unread_battery_clears_it() -> Result<(), Box<dyn std::error::Error>> {
        let mut indicator = FakeIndicator::default();
        let mut warning = Warning::unset();
        warning.tick(
            Some(reading(10, ChargeStatus::Discharging)?),
            &mut indicator,
        )?;
        warning.tick(None, &mut indicator)?;
        warning.tick(None, &mut indicator)?;
        assert_eq!(indicator.calls, [Call::Warn, Call::Clear]);
        assert!(!warning.shown());
        Ok(())
    }

    #[test]
    fn a_failing_indicator_is_left_alone_until_the_battery_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut indicator = FakeIndicator::failing();
        let mut warning = Warning::unset();
        let low = Some(reading(10, ChargeStatus::Discharging)?);
        assert!(warning.tick(low, &mut indicator).is_err());
        assert_eq!(warning.tick(low, &mut indicator), Ok(()));
        assert!(!warning.shown());
        let plugged = Some(reading(10, ChargeStatus::Charging)?);
        assert!(warning.tick(plugged, &mut indicator).is_err());
        assert_eq!(indicator.calls, [Call::Warn, Call::Clear]);
        Ok(())
    }
}
