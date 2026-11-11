use std::fmt;

pub const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_MINUTE: i64 = 60;
const SECONDS_PER_HOUR: i64 = 3_600;
const LARGEST_OFFSET: i32 = 18 * 3_600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixSeconds(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    Hour(u8),
    Minute(u8),
    Offset(i32),
}

impl fmt::Display for TimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hour(hour) => write!(formatter, "hour {hour} is not in 0..=23"),
            Self::Minute(minute) => write!(formatter, "minute {minute} is not in 0..=59"),
            Self::Offset(seconds) => {
                write!(
                    formatter,
                    "UTC offset {seconds}s is beyond ±{LARGEST_OFFSET}s"
                )
            }
        }
    }
}

impl std::error::Error for TimeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
}

impl TimeOfDay {
    pub fn new(hour: u8, minute: u8) -> Result<Self, TimeError> {
        if hour > 23 {
            return Err(TimeError::Hour(hour));
        }
        if minute > 59 {
            return Err(TimeError::Minute(minute));
        }
        Ok(Self { hour, minute })
    }

    #[must_use]
    pub fn hour(self) -> u8 {
        self.hour
    }

    #[must_use]
    pub fn minute(self) -> u8 {
        self.minute
    }

    #[must_use]
    pub fn seconds_into_day(self) -> i64 {
        i64::from(self.hour) * SECONDS_PER_HOUR + i64::from(self.minute) * SECONDS_PER_MINUTE
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    #[must_use]
    pub fn of_day(days_since_epoch: i64) -> Self {
        match (days_since_epoch + 3).rem_euclid(7) {
            0 => Self::Monday,
            1 => Self::Tuesday,
            2 => Self::Wednesday,
            3 => Self::Thursday,
            4 => Self::Friday,
            5 => Self::Saturday,
            _ => Self::Sunday,
        }
    }

    #[must_use]
    pub fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UtcOffset(i32);

impl UtcOffset {
    pub const UTC: Self = Self(0);

    pub fn from_seconds(seconds: i32) -> Result<Self, TimeError> {
        if seconds.abs() > LARGEST_OFFSET {
            return Err(TimeError::Offset(seconds));
        }
        Ok(Self(seconds))
    }

    #[must_use]
    pub fn seconds(self) -> i64 {
        i64::from(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_was_a_thursday() {
        assert_eq!(Weekday::of_day(0), Weekday::Thursday);
    }

    #[test]
    fn days_before_the_epoch_keep_the_cycle() {
        assert_eq!(Weekday::of_day(-1), Weekday::Wednesday);
        assert_eq!(Weekday::of_day(-7), Weekday::Thursday);
    }

    #[test]
    fn a_known_date_has_its_weekday() {
        assert_eq!(Weekday::of_day(20_768), Weekday::Wednesday);
    }

    #[test]
    fn an_hour_past_23_is_refused() {
        assert_eq!(TimeOfDay::new(24, 0), Err(TimeError::Hour(24)));
    }

    #[test]
    fn a_minute_past_59_is_refused() {
        assert_eq!(TimeOfDay::new(7, 60), Err(TimeError::Minute(60)));
    }

    #[test]
    fn a_time_of_day_counts_its_seconds() -> Result<(), TimeError> {
        assert_eq!(TimeOfDay::new(6, 45)?.seconds_into_day(), 24_300);
        Ok(())
    }

    #[test]
    fn an_offset_beyond_eighteen_hours_is_refused() {
        assert_eq!(
            UtcOffset::from_seconds(18 * 3_600 + 1),
            Err(TimeError::Offset(64_801))
        );
    }
}
