#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Boot {
    Alarm,
    Press,
    Charger,
    Other,
}

impl Boot {
    #[must_use]
    pub fn lit(self) -> bool {
        self != Self::Alarm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_boot_by_the_alarm_starts_dark() {
        assert!(!Boot::Alarm.lit());
        assert!(Boot::Press.lit());
        assert!(Boot::Charger.lit());
        assert!(Boot::Other.lit());
    }
}
