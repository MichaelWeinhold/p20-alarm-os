use crate::input::{InputEvent, Key};
use crate::ringer::Ring;
use crate::time::UnixSeconds;

pub const LIT_SECONDS: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Light {
    seconds: i64,
    until: Option<UnixSeconds>,
}

impl Light {
    #[must_use]
    pub fn dark(seconds: i64) -> Self {
        Self {
            seconds,
            until: None,
        }
    }

    #[must_use]
    pub fn lit_at(now: UnixSeconds, seconds: i64) -> Self {
        let mut light = Self::dark(seconds);
        light.relit(now);
        light
    }

    #[must_use]
    pub fn lit(&self) -> bool {
        self.until.is_some()
    }

    #[must_use]
    pub fn seconds_left(&self, now: UnixSeconds) -> Option<u64> {
        self.until
            .map(|until| until.0.saturating_sub(now.0).max(1).unsigned_abs())
    }

    pub fn tick(&mut self, now: UnixSeconds, ring: Ring) {
        if ring == Ring::Ringing {
            self.relit(now);
        } else if self.until.is_some_and(|until| now >= until) {
            self.until = None;
        }
    }

    pub fn heard(&mut self, event: InputEvent, ring: Ring, now: UnixSeconds) {
        match (self.lit(), event, ring) {
            (true, InputEvent::Press(Key::Power), Ring::Quiet) => self.until = None,
            (true, _, _) | (false, InputEvent::Press(Key::Power), _) => self.relit(now),
            (false, _, _) => {}
        }
    }

    fn relit(&mut self, now: UnixSeconds) {
        self.until = Some(UnixSeconds(now.0.saturating_add(self.seconds)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: UnixSeconds = UnixSeconds(1_794_512_278);
    const POWER: InputEvent = InputEvent::Press(Key::Power);
    const VOLUME: InputEvent = InputEvent::Press(Key::VolumeDown);
    const TOUCH: InputEvent = InputEvent::Touch { x: 509, y: 1100 };

    fn at(seconds: i64) -> UnixSeconds {
        UnixSeconds(START.0 + seconds)
    }

    fn dark() -> Light {
        Light::dark(LIT_SECONDS)
    }

    #[test]
    fn the_panel_is_lit_at_start_and_goes_off_after_its_seconds() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        assert!(light.lit());
        assert_eq!(light.seconds_left(START), Some(10));
        light.tick(at(9), Ring::Quiet);
        assert!(light.lit());
        assert_eq!(light.seconds_left(at(9)), Some(1));
        light.tick(at(10), Ring::Quiet);
        assert!(!light.lit());
        assert_eq!(light.seconds_left(at(10)), None);
    }

    #[test]
    fn a_press_on_a_dark_panel_lights_it_for_its_seconds() {
        let mut light = dark();
        light.heard(POWER, Ring::Quiet, at(30));
        assert!(light.lit());
        assert_eq!(light.seconds_left(at(30)), Some(10));
        light.tick(at(40), Ring::Quiet);
        assert!(!light.lit());
    }

    #[test]
    fn a_touch_or_a_volume_key_keeps_a_lit_panel_lit() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        light.heard(TOUCH, Ring::Quiet, at(8));
        light.tick(at(17), Ring::Quiet);
        assert!(light.lit());
        light.heard(VOLUME, Ring::Quiet, at(17));
        light.tick(at(26), Ring::Quiet);
        assert!(light.lit());
        light.tick(at(27), Ring::Quiet);
        assert!(!light.lit());
    }

    #[test]
    fn a_touch_or_a_volume_key_leaves_a_dark_panel_dark() {
        let mut light = dark();
        light.heard(TOUCH, Ring::Quiet, at(30));
        light.heard(VOLUME, Ring::Snoozed(at(300)), at(31));
        assert_eq!(light, dark());
    }

    #[test]
    fn a_press_on_a_lit_panel_turns_it_off_at_once() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        light.heard(POWER, Ring::Quiet, at(3));
        assert!(!light.lit());
    }

    #[test]
    fn a_ringing_alarm_lights_the_panel_and_holds_it_lit() {
        let mut light = dark();
        light.tick(at(60), Ring::Ringing);
        assert!(light.lit());
        light.tick(at(600), Ring::Ringing);
        assert_eq!(light.seconds_left(at(600)), Some(10));
        light.tick(at(609), Ring::Quiet);
        assert!(light.lit());
        light.tick(at(610), Ring::Quiet);
        assert!(!light.lit());
    }

    #[test]
    fn a_press_that_dismisses_or_ends_a_snooze_leaves_the_panel_lit() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        light.heard(POWER, Ring::Ringing, at(5));
        assert_eq!(light.seconds_left(at(5)), Some(10));
        light.heard(POWER, Ring::Snoozed(at(300)), at(6));
        assert_eq!(light.seconds_left(at(6)), Some(10));
    }

    #[test]
    fn the_seconds_are_a_setting() {
        let mut light = Light::lit_at(START, 30);
        light.tick(at(29), Ring::Quiet);
        assert!(light.lit());
        light.tick(at(30), Ring::Quiet);
        assert!(!light.lit());
    }
}
