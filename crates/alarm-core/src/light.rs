use crate::face::Screen;
use crate::input::{InputEvent, Key};
use crate::ringer::Ring;
use crate::time::UnixSeconds;

pub const LIT_CHOICES: [i64; 5] = [10, 30, 60, 120, 300];
pub const LIT_SECONDS: i64 = LIT_CHOICES[0];

#[must_use]
pub fn next_lit_seconds(seconds: i64) -> i64 {
    LIT_CHOICES
        .into_iter()
        .find(|&choice| choice > seconds)
        .unwrap_or(LIT_SECONDS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Light {
    seconds: i64,
    until: Option<UnixSeconds>,
    screen: Screen,
}

impl Light {
    #[must_use]
    pub fn dark(seconds: i64) -> Self {
        Self {
            seconds,
            until: None,
            screen: Screen::Alarm,
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
    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn open(&mut self, screen: Screen) {
        self.screen = screen;
    }

    pub fn set_seconds(&mut self, seconds: i64) {
        self.seconds = seconds;
    }

    #[must_use]
    pub fn seconds_left(&self, now: UnixSeconds) -> Option<u64> {
        self.until
            .map(|until| until.0.saturating_sub(now.0).max(1).unsigned_abs())
    }

    pub fn tick(&mut self, now: UnixSeconds, ring: Ring) {
        if ring == Ring::Ringing {
            self.relit(now);
            self.screen = Screen::Alarm;
        } else if self.until.is_some_and(|until| now >= until) {
            self.darken();
        }
    }

    pub fn heard(&mut self, event: InputEvent, ring: Ring, now: UnixSeconds) {
        match (self.lit(), event, ring) {
            (true, InputEvent::Press(Key::Power), Ring::Quiet) => self.darken(),
            (true, _, _) | (false, InputEvent::Press(Key::Power), _) => self.relit(now),
            (false, _, _) => {}
        }
    }

    fn darken(&mut self) {
        self.until = None;
        self.screen = Screen::Alarm;
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
    fn a_panel_that_goes_dark_or_rings_comes_back_on_the_alarm_screen() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        light.open(Screen::Timer);
        light.tick(at(10), Ring::Quiet);
        light.heard(POWER, Ring::Quiet, at(11));
        assert_eq!(light.screen(), Screen::Alarm);
        light.open(Screen::Settings);
        light.heard(POWER, Ring::Quiet, at(12));
        light.heard(POWER, Ring::Quiet, at(13));
        assert_eq!(light.screen(), Screen::Alarm);
        light.open(Screen::Timer);
        light.tick(at(14), Ring::Ringing);
        assert_eq!(light.screen(), Screen::Alarm);
    }

    #[test]
    fn an_open_screen_stays_open_while_the_panel_is_lit() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        light.open(Screen::Settings);
        light.heard(TOUCH, Ring::Quiet, at(8));
        light.tick(at(17), Ring::Quiet);
        assert_eq!(light.screen(), Screen::Settings);
    }

    #[test]
    fn the_seconds_are_a_setting() {
        let mut light = Light::lit_at(START, 30);
        light.tick(at(29), Ring::Quiet);
        assert!(light.lit());
        light.tick(at(30), Ring::Quiet);
        assert!(!light.lit());
    }

    #[test]
    fn a_new_setting_lasts_from_the_next_relight() {
        let mut light = Light::lit_at(START, LIT_SECONDS);
        light.set_seconds(60);
        light.tick(at(10), Ring::Quiet);
        assert!(!light.lit());
        light.heard(POWER, Ring::Quiet, at(20));
        light.tick(at(79), Ring::Quiet);
        assert!(light.lit());
        light.tick(at(80), Ring::Quiet);
        assert!(!light.lit());
    }

    #[test]
    fn the_choices_step_up_and_wrap_round_to_the_default() {
        let mut seen = vec![LIT_SECONDS];
        for _ in LIT_CHOICES {
            seen.push(next_lit_seconds(seen[seen.len() - 1]));
        }
        assert_eq!(seen, [10, 30, 60, 120, 300, 10]);
        assert_eq!(next_lit_seconds(45), 60);
        assert_eq!(next_lit_seconds(i64::MAX), LIT_SECONDS);
    }
}
