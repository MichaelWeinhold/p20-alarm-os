use crate::ringer::Ring;
use crate::time::UnixSeconds;

pub const WINDOW_SECONDS: i64 = 2 * 60;
pub const POWER_ON_LEAD_SECONDS: i64 = 2 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Answer {
    Lit,
    Suspend(UnixSeconds),
    PowerOff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Window {
    seconds: i64,
    ends: Option<UnixSeconds>,
}

impl Window {
    #[must_use]
    pub fn lit(seconds: i64) -> Self {
        Self {
            seconds,
            ends: None,
        }
    }

    #[must_use]
    pub fn running(&self) -> bool {
        self.ends.is_some()
    }

    pub fn dark(&mut self, now: UnixSeconds) {
        if self.ends.is_none() {
            self.ends = Some(UnixSeconds(now.0.saturating_add(self.seconds)));
        }
    }

    pub fn light(&mut self) {
        self.ends = None;
    }

    #[must_use]
    pub fn answer(&self, now: UnixSeconds, ring: Ring, next: Option<UnixSeconds>) -> Answer {
        let Some(ends) = self.ends else {
            return Answer::Lit;
        };
        let due = match ring {
            Ring::Quiet => next,
            Ring::Ringing => Some(now),
            Ring::Snoozed(until) => Some(next.map_or(until, |next| next.min(until))),
        };
        if now < ends {
            return Answer::Suspend(due.map_or(ends, |due| due.min(ends)));
        }
        match due {
            Some(due) if ring != Ring::Quiet => Answer::Suspend(due),
            Some(due) if due.0 <= ends.0.saturating_add(POWER_ON_LEAD_SECONDS) => {
                Answer::Suspend(due)
            }
            _ => Answer::PowerOff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: UnixSeconds = UnixSeconds(1_794_512_278);

    fn at(seconds: i64) -> UnixSeconds {
        UnixSeconds(START.0 + seconds)
    }

    fn dark_at(seconds: i64) -> Window {
        let mut window = Window::lit(WINDOW_SECONDS);
        window.dark(at(seconds));
        window
    }

    #[test]
    fn the_window_ends_two_minutes_after_the_panel_goes_off_and_answers_power_off() {
        let window = dark_at(0);
        assert!(window.running());
        assert_eq!(
            window.answer(at(0), Ring::Quiet, None),
            Answer::Suspend(at(120))
        );
        assert_eq!(
            window.answer(at(119), Ring::Quiet, None),
            Answer::Suspend(at(120))
        );
        assert_eq!(window.answer(at(120), Ring::Quiet, None), Answer::PowerOff);
    }

    #[test]
    fn a_press_in_the_window_lights_the_time_and_ends_it() {
        let mut window = dark_at(0);
        window.light();
        assert!(!window.running());
        assert_eq!(window.answer(at(60), Ring::Quiet, None), Answer::Lit);
        assert_eq!(window.answer(at(120), Ring::Quiet, None), Answer::Lit);
        assert_eq!(window.answer(at(500), Ring::Quiet, None), Answer::Lit);
    }

    #[test]
    fn the_next_window_starts_when_the_panel_goes_off_again() {
        let mut window = dark_at(0);
        window.light();
        window.dark(at(70));
        assert_eq!(
            window.answer(at(120), Ring::Quiet, None),
            Answer::Suspend(at(190))
        );
        assert_eq!(window.answer(at(190), Ring::Quiet, None), Answer::PowerOff);
    }

    #[test]
    fn a_press_after_the_window_does_not_take_back_its_answer() {
        let mut window = dark_at(0);
        assert_eq!(window.answer(at(120), Ring::Quiet, None), Answer::PowerOff);
        window.dark(at(125));
        assert_eq!(window.answer(at(125), Ring::Quiet, None), Answer::PowerOff);
        window.light();
        assert_eq!(window.answer(at(130), Ring::Quiet, None), Answer::Lit);
    }

    #[test]
    fn the_window_does_not_run_while_the_panel_is_lit() {
        let mut window = Window::lit(WINDOW_SECONDS);
        assert!(!window.running());
        assert_eq!(window.answer(at(600), Ring::Quiet, None), Answer::Lit);
        window.dark(at(600));
        assert_eq!(
            window.answer(at(700), Ring::Quiet, None),
            Answer::Suspend(at(720))
        );
        assert_eq!(window.answer(at(720), Ring::Quiet, None), Answer::PowerOff);
    }

    #[test]
    fn an_alarm_inside_the_window_wakes_the_suspend_for_it() {
        let window = dark_at(0);
        assert_eq!(
            window.answer(at(10), Ring::Quiet, Some(at(60))),
            Answer::Suspend(at(60))
        );
    }

    #[test]
    fn an_alarm_due_within_two_minutes_of_the_window_end_keeps_it_suspended() {
        let window = dark_at(0);
        assert_eq!(
            window.answer(at(120), Ring::Quiet, Some(at(200))),
            Answer::Suspend(at(200))
        );
        assert_eq!(
            window.answer(at(120), Ring::Quiet, Some(at(240))),
            Answer::Suspend(at(240))
        );
    }

    #[test]
    fn an_alarm_due_later_than_that_answers_power_off() {
        let window = dark_at(0);
        assert_eq!(
            window.answer(at(120), Ring::Quiet, Some(at(241))),
            Answer::PowerOff
        );
    }

    #[test]
    fn a_snoozed_alarm_keeps_it_suspended_until_the_snooze_ends() {
        let window = dark_at(0);
        assert_eq!(
            window.answer(at(120), Ring::Snoozed(at(540)), Some(at(86_400))),
            Answer::Suspend(at(540))
        );
        assert_eq!(
            window.answer(at(60), Ring::Snoozed(at(540)), None),
            Answer::Suspend(at(120))
        );
    }

    #[test]
    fn a_ringing_alarm_keeps_it_awake() {
        let window = dark_at(0);
        assert_eq!(
            window.answer(at(130), Ring::Ringing, Some(at(86_400))),
            Answer::Suspend(at(130))
        );
    }

    #[test]
    fn going_dark_twice_does_not_restart_the_window() {
        let mut window = dark_at(0);
        window.dark(at(100));
        assert_eq!(window.answer(at(120), Ring::Quiet, None), Answer::PowerOff);
    }

    #[test]
    fn the_two_minutes_are_a_setting() {
        let mut window = Window::lit(300);
        window.dark(at(0));
        assert_eq!(
            window.answer(at(299), Ring::Quiet, None),
            Answer::Suspend(at(300))
        );
        assert_eq!(window.answer(at(300), Ring::Quiet, None), Answer::PowerOff);
        assert_eq!(
            window.answer(at(300), Ring::Quiet, Some(at(420))),
            Answer::Suspend(at(420))
        );
    }
}
