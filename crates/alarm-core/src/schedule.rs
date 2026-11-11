use crate::alarm::{Alarm, earliest};
use crate::hardware::{Clock, HardwareError, WakeAlarm};
use crate::time::{UnixSeconds, UtcOffset};

pub fn arm_next(
    alarms: &[Alarm],
    offset: UtcOffset,
    clock: &impl Clock,
    wake: &mut impl WakeAlarm,
) -> Result<Option<UnixSeconds>, HardwareError> {
    let now = clock.now()?;
    if let Some(at) = earliest(alarms, now, offset) {
        wake.arm(at)?;
        Ok(Some(at))
    } else {
        wake.disarm()?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{SECONDS_PER_DAY, TimeError, TimeOfDay};

    const NOON: UnixSeconds = UnixSeconds(20_768 * SECONDS_PER_DAY + 12 * 3_600);

    struct FixedClock(Option<UnixSeconds>);

    impl Clock for FixedClock {
        fn now(&self) -> Result<UnixSeconds, HardwareError> {
            self.0.ok_or(HardwareError::Absent("rtc0"))
        }
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    enum Rtc {
        #[default]
        Untouched,
        Armed(UnixSeconds),
        Disarmed,
    }

    impl WakeAlarm for Rtc {
        fn arm(&mut self, at: UnixSeconds) -> Result<(), HardwareError> {
            *self = Self::Armed(at);
            Ok(())
        }

        fn disarm(&mut self) -> Result<(), HardwareError> {
            *self = Self::Disarmed;
            Ok(())
        }
    }

    #[test]
    fn the_earliest_alarm_is_armed_on_the_rtc() -> Result<(), Box<dyn std::error::Error>> {
        let mut rtc = Rtc::default();
        let alarms = [Alarm::once(TimeOfDay::new(13, 0)?)];
        let armed = arm_next(&alarms, UtcOffset::UTC, &FixedClock(Some(NOON)), &mut rtc)?;
        let expected = UnixSeconds(NOON.0 + 3_600);
        assert_eq!(armed, Some(expected));
        assert_eq!(rtc, Rtc::Armed(expected));
        Ok(())
    }

    #[test]
    fn with_no_alarms_the_rtc_is_disarmed() -> Result<(), HardwareError> {
        let mut rtc = Rtc::Armed(NOON);
        let armed = arm_next(&[], UtcOffset::UTC, &FixedClock(Some(NOON)), &mut rtc)?;
        assert_eq!(armed, None);
        assert_eq!(rtc, Rtc::Disarmed);
        Ok(())
    }

    #[test]
    fn an_absent_clock_arms_nothing() -> Result<(), TimeError> {
        let mut rtc = Rtc::default();
        let alarms = [Alarm::once(TimeOfDay::new(13, 0)?)];
        let clock = FixedClock(None);
        assert_eq!(
            arm_next(&alarms, UtcOffset::UTC, &clock, &mut rtc),
            Err(HardwareError::Absent("rtc0"))
        );
        assert_eq!(rtc, Rtc::Untouched);
        Ok(())
    }
}
