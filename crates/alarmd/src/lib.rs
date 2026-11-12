use alarm_core::battery::{ChargeLimit, ChargeStep, stop_at_limit, wants_mark};
use alarm_core::face::show_time;
use alarm_core::hardware::{Battery, Clock, Display, HardwareError};
use alarm_core::time::{UtcOffset, seconds_to_next_minute};
use pid1::kmsg;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const LOCAL_OFFSET_SECONDS: i32 = 3_600;
pub const BRIGHTNESS: u32 = 2_000;
pub const RETRY_SECONDS: u64 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Minute {
    pub log: Vec<String>,
    pub sleep_seconds: u64,
}

pub fn minute(
    clock: &impl Clock,
    offset: UtcOffset,
    battery: &mut Result<impl Battery, HardwareError>,
    display: &mut impl Display,
) -> Minute {
    let mut log = Vec::new();
    let charge = match battery {
        Ok(battery) => stop_at_limit(ChargeLimit::DEFAULT, battery),
        Err(error) => Err(error.clone()),
    };
    match charge {
        Ok(ChargeStep::Stopped) => log.push(kmsg::line("charge stopped at the limit")),
        Ok(ChargeStep::Left) => {}
        Err(error) => log.push(kmsg::failure("holding the charge limit", &error)),
    }
    let low = battery.as_ref().map_or(true, wants_mark);
    let sleep_seconds = match show_time(clock, offset, low, display) {
        Ok(now) => seconds_to_next_minute(now),
        Err(error) => {
            log.push(kmsg::failure("showing the time", &error));
            RETRY_SECONDS
        }
    };
    Minute { log, sleep_seconds }
}

#[cfg(test)]
mod tests {
    use alarm_core::battery::{BatteryReading, ChargeStatus, Health, Percent};
    use alarm_core::face::{Frame, Size, WARNING, render};
    use alarm_core::time::{TimeError, TimeOfDay, UnixSeconds};

    use super::*;

    const SIZE: Size = Size {
        width: 100,
        height: 64,
    };

    struct FixedClock(Result<UnixSeconds, HardwareError>);

    impl Clock for FixedClock {
        fn now(&self) -> Result<UnixSeconds, HardwareError> {
            self.0.clone()
        }
    }

    struct FakeBattery {
        capacity: Percent,
        status: ChargeStatus,
        enabled: bool,
    }

    impl Battery for FakeBattery {
        fn read(&self) -> Result<BatteryReading, HardwareError> {
            Ok(BatteryReading {
                capacity: self.capacity,
                status: self.status,
                microvolts: 4_000_000,
                decicelsius: 250,
                milliamps: 100,
                health: Health::Good,
            })
        }

        fn charger_enabled(&self) -> Result<bool, HardwareError> {
            Ok(self.enabled)
        }

        fn set_charger_enabled(&mut self, enabled: bool) -> Result<(), HardwareError> {
            self.enabled = enabled;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeDisplay {
        shown: Vec<Frame>,
    }

    impl Display for FakeDisplay {
        fn size(&self) -> Size {
            SIZE
        }

        fn show(&mut self, frame: &Frame) -> Result<(), HardwareError> {
            self.shown.push(frame.clone());
            Ok(())
        }
    }

    fn battery(capacity: u8, status: ChargeStatus) -> Result<FakeBattery, HardwareError> {
        let capacity = Percent::new(capacity).map_err(|error| HardwareError::Failed {
            device: "fake",
            reason: error.to_string(),
        })?;
        Ok(FakeBattery {
            capacity,
            status,
            enabled: true,
        })
    }

    fn at(hour: i64, minute: i64, second: i64) -> FixedClock {
        FixedClock(Ok(UnixSeconds(
            20_768 * 86_400 + hour * 3_600 + minute * 60 + second,
        )))
    }

    #[test]
    fn a_minute_draws_the_local_time_and_sleeps_to_the_next()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let offset = UtcOffset::from_seconds(LOCAL_OFFSET_SECONDS)?;
        let step = minute(&at(19, 37, 58), offset, &mut battery, &mut display);
        assert_eq!(
            step,
            Minute {
                log: Vec::new(),
                sleep_seconds: 2,
            }
        );
        assert_eq!(
            display.shown,
            [render(SIZE, TimeOfDay::new(20, 37)?, false)]
        );
        Ok(())
    }

    #[test]
    fn at_the_limit_the_charge_stops_and_says_so() -> Result<(), HardwareError> {
        let mut battery = battery(80, ChargeStatus::Charging);
        let mut display = FakeDisplay::default();
        let step = minute(&at(6, 0, 0), UtcOffset::UTC, &mut battery, &mut display);
        assert_eq!(step.log, [kmsg::line("charge stopped at the limit")]);
        assert!(!battery?.enabled);
        Ok(())
    }

    #[test]
    fn a_low_battery_puts_the_mark_on_the_face() {
        let mut battery = battery(20, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        minute(&at(6, 0, 0), UtcOffset::UTC, &mut battery, &mut display);
        assert!(
            display
                .shown
                .iter()
                .all(|frame| (0..SIZE.height).any(|y| frame.row(y).contains(&WARNING)))
        );
        assert_eq!(display.shown.len(), 1);
    }

    #[test]
    fn an_absent_battery_still_shows_the_time_with_the_mark() -> Result<(), TimeError> {
        let mut battery: Result<FakeBattery, HardwareError> =
            Err(HardwareError::Absent("Battery/capacity"));
        let mut display = FakeDisplay::default();
        let step = minute(&at(6, 0, 30), UtcOffset::UTC, &mut battery, &mut display);
        assert_eq!(
            step.log,
            [kmsg::failure(
                "holding the charge limit",
                &HardwareError::Absent("Battery/capacity")
            )]
        );
        assert_eq!(step.sleep_seconds, 30);
        assert_eq!(display.shown, [render(SIZE, TimeOfDay::new(6, 0)?, true)]);
        Ok(())
    }

    #[test]
    fn a_failing_clock_draws_nothing_and_retries_soon() {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let clock = FixedClock(Err(HardwareError::Absent("realtime clock")));
        let step = minute(&clock, UtcOffset::UTC, &mut battery, &mut display);
        assert_eq!(
            step,
            Minute {
                log: vec![kmsg::failure(
                    "showing the time",
                    &HardwareError::Absent("realtime clock")
                )],
                sleep_seconds: RETRY_SECONDS,
            }
        );
        assert_eq!(display.shown, Vec::<Frame>::new());
    }
}
