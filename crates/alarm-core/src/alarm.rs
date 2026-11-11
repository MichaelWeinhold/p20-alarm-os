use crate::time::{SECONDS_PER_DAY, TimeOfDay, UnixSeconds, UtcOffset, Weekday};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Weekdays(u8);

impl Weekdays {
    pub const NONE: Self = Self(0);
    pub const EVERY_DAY: Self = Self(0b111_1111);
    pub const WORKDAYS: Self = Self(0b001_1111);

    #[must_use]
    pub fn with(self, day: Weekday) -> Self {
        Self(self.0 | day.bit())
    }

    #[must_use]
    pub fn contains(self, day: Weekday) -> bool {
        self.0 & day.bit() != 0
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Alarm {
    pub time: TimeOfDay,
    pub repeat: Weekdays,
}

impl Alarm {
    #[must_use]
    pub fn once(time: TimeOfDay) -> Self {
        Self {
            time,
            repeat: Weekdays::NONE,
        }
    }

    #[must_use]
    pub fn next_after(&self, now: UnixSeconds, offset: UtcOffset) -> UnixSeconds {
        let local = now.0 + offset.seconds();
        let today = local.div_euclid(SECONDS_PER_DAY);
        let mut day = today;
        loop {
            let candidate = day * SECONDS_PER_DAY + self.time.seconds_into_day();
            let wanted = self.repeat.is_empty() || self.repeat.contains(Weekday::of_day(day));
            if candidate > local && wanted {
                return UnixSeconds(candidate - offset.seconds());
            }
            day += 1;
        }
    }
}

#[must_use]
pub fn earliest(alarms: &[Alarm], now: UnixSeconds, offset: UtcOffset) -> Option<UnixSeconds> {
    alarms
        .iter()
        .map(|alarm| alarm.next_after(now, offset))
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TimeError;

    const WEDNESDAY_MIDNIGHT: i64 = 20_768 * SECONDS_PER_DAY;

    fn at(hour: u8, minute: u8) -> Result<TimeOfDay, TimeError> {
        TimeOfDay::new(hour, minute)
    }

    fn hours(count: i64) -> i64 {
        count * 3_600
    }

    #[test]
    fn a_one_shot_later_today_fires_today() -> Result<(), TimeError> {
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(6));
        let fires = Alarm::once(at(7, 0)?).next_after(now, UtcOffset::UTC);
        assert_eq!(fires, UnixSeconds(WEDNESDAY_MIDNIGHT + hours(7)));
        Ok(())
    }

    #[test]
    fn a_one_shot_already_past_fires_tomorrow() -> Result<(), TimeError> {
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(8));
        let fires = Alarm::once(at(7, 0)?).next_after(now, UtcOffset::UTC);
        assert_eq!(fires, UnixSeconds(WEDNESDAY_MIDNIGHT + hours(24 + 7)));
        Ok(())
    }

    #[test]
    fn an_alarm_at_exactly_now_fires_next_time_not_now() -> Result<(), TimeError> {
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(7));
        let fires = Alarm::once(at(7, 0)?).next_after(now, UtcOffset::UTC);
        assert_eq!(fires, UnixSeconds(WEDNESDAY_MIDNIGHT + hours(24 + 7)));
        Ok(())
    }

    #[test]
    fn a_workday_alarm_on_friday_evening_fires_on_monday() -> Result<(), TimeError> {
        let friday_evening = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(2 * 24 + 20));
        let alarm = Alarm {
            time: at(6, 30)?,
            repeat: Weekdays::WORKDAYS,
        };
        let monday = WEDNESDAY_MIDNIGHT + hours(5 * 24) + hours(6) + 30 * 60;
        assert_eq!(
            alarm.next_after(friday_evening, UtcOffset::UTC),
            UnixSeconds(monday)
        );
        Ok(())
    }

    #[test]
    fn a_single_weekday_waits_a_full_week_once_it_has_passed() -> Result<(), TimeError> {
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(9));
        let alarm = Alarm {
            time: at(8, 0)?,
            repeat: Weekdays::NONE.with(Weekday::Wednesday),
        };
        assert_eq!(
            alarm.next_after(now, UtcOffset::UTC),
            UnixSeconds(WEDNESDAY_MIDNIGHT + hours(7 * 24 + 8))
        );
        Ok(())
    }

    #[test]
    fn the_offset_moves_the_alarm_to_local_time() -> Result<(), TimeError> {
        let berlin_winter = UtcOffset::from_seconds(3_600)?;
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT);
        let fires = Alarm::once(at(7, 0)?).next_after(now, berlin_winter);
        assert_eq!(fires, UnixSeconds(WEDNESDAY_MIDNIGHT + hours(6)));
        Ok(())
    }

    #[test]
    fn the_local_day_decides_the_weekday_not_the_utc_day() -> Result<(), TimeError> {
        let late_tuesday_utc = UnixSeconds(WEDNESDAY_MIDNIGHT - hours(1));
        let plus_two = UtcOffset::from_seconds(7_200)?;
        let alarm = Alarm {
            time: at(0, 30)?,
            repeat: Weekdays::NONE.with(Weekday::Wednesday),
        };
        assert_eq!(
            alarm.next_after(late_tuesday_utc, plus_two),
            UnixSeconds(WEDNESDAY_MIDNIGHT + hours(7 * 24) + 30 * 60 - hours(2))
        );
        Ok(())
    }

    #[test]
    fn every_single_weekday_fires_within_a_week() -> Result<(), TimeError> {
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(12));
        for day in Weekday::ALL {
            let alarm = Alarm {
                time: at(12, 0)?,
                repeat: Weekdays::NONE.with(day),
            };
            let fires = alarm.next_after(now, UtcOffset::UTC);
            assert!(fires.0 > now.0 && fires.0 <= now.0 + 7 * SECONDS_PER_DAY);
        }
        Ok(())
    }

    #[test]
    fn earliest_picks_the_soonest_alarm() -> Result<(), TimeError> {
        let now = UnixSeconds(WEDNESDAY_MIDNIGHT + hours(6));
        let alarms = [Alarm::once(at(9, 0)?), Alarm::once(at(6, 30)?)];
        assert_eq!(
            earliest(&alarms, now, UtcOffset::UTC),
            Some(UnixSeconds(WEDNESDAY_MIDNIGHT + hours(6) + 30 * 60))
        );
        Ok(())
    }

    #[test]
    fn no_alarms_means_nothing_to_wake_for() {
        assert_eq!(
            earliest(&[], UnixSeconds(WEDNESDAY_MIDNIGHT), UtcOffset::UTC),
            None
        );
    }
}
