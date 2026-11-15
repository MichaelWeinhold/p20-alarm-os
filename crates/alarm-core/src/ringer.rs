use crate::alarm::{Alarm, Weekdays};
use crate::face::{Face, Size, Step, Target, render, target};
use crate::hardware::{Clock, Display, HardwareError, Input};
use crate::input::{InputEvent, Key};
use crate::light::Light;
use crate::time::{TimeOfDay, UnixSeconds, UtcOffset, start_of_minute};

pub const SNOOZE_SECONDS: i64 = 9 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ring {
    Quiet,
    Ringing,
    Snoozed(UnixSeconds),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ringer {
    alarm: TimeOfDay,
    next: Option<UnixSeconds>,
    ring: Ring,
}

fn delta(step: Step) -> i8 {
    match step {
        Step::Up => 1,
        Step::Down => -1,
    }
}

impl Ringer {
    #[must_use]
    pub fn new(alarm: TimeOfDay) -> Self {
        Self {
            alarm,
            next: None,
            ring: Ring::Quiet,
        }
    }

    #[must_use]
    pub fn alarm(&self) -> TimeOfDay {
        self.alarm
    }

    #[must_use]
    pub fn armed(&self) -> bool {
        self.next.is_some()
    }

    #[must_use]
    pub fn ring(&self) -> Ring {
        self.ring
    }

    fn after(&self, now: UnixSeconds, offset: UtcOffset) -> UnixSeconds {
        Alarm {
            time: self.alarm,
            repeat: Weekdays::EVERY_DAY,
        }
        .next_after(now, offset)
    }

    pub fn tick(&mut self, now: UnixSeconds, offset: UtcOffset) {
        if let Some(next) = self.next
            && now >= next
        {
            self.ring = Ring::Ringing;
            self.next = Some(self.after(now, offset));
        }
        if let Ring::Snoozed(until) = self.ring
            && now >= until
        {
            self.ring = Ring::Ringing;
        }
    }

    pub fn heard(&mut self, event: InputEvent, size: Size, now: UnixSeconds, offset: UtcOffset) {
        match (self.ring, event) {
            (Ring::Ringing, InputEvent::Press(Key::VolumeUp | Key::VolumeDown)) => {
                self.ring = Ring::Snoozed(UnixSeconds(start_of_minute(now).0 + SNOOZE_SECONDS));
            }
            (Ring::Ringing | Ring::Snoozed(_), InputEvent::Press(Key::Power)) => {
                self.ring = Ring::Quiet;
            }
            (Ring::Quiet | Ring::Snoozed(_), InputEvent::Touch { x, y }) => {
                self.touched(target(size, x, y), now, offset);
            }
            _ => {}
        }
    }

    fn touched(&mut self, target: Target, now: UnixSeconds, offset: UtcOffset) {
        match target {
            Target::Hour(step) => self.alarm = self.alarm.stepped_hour(delta(step)),
            Target::Minute(step) => self.alarm = self.alarm.stepped_minute(delta(step)),
            Target::Elsewhere if self.armed() => {
                self.next = None;
                self.ring = Ring::Quiet;
                return;
            }
            Target::Elsewhere => self.next = Some(self.after(now, offset)),
        }
        if self.armed() {
            self.next = Some(self.after(now, offset));
        }
    }

    #[must_use]
    pub fn face(&self, now: UnixSeconds, offset: UtcOffset, low_battery: bool) -> Face {
        Face {
            time: TimeOfDay::at(now, offset),
            alarm: self.alarm,
            armed: self.armed(),
            ringing: self.ring == Ring::Ringing,
            low_battery,
        }
    }
}

pub fn show(
    clock: &impl Clock,
    offset: UtcOffset,
    ringer: &mut Ringer,
    light: &mut Light,
    low_battery: bool,
    display: &mut impl Display,
) -> Result<UnixSeconds, HardwareError> {
    let now = clock.now()?;
    ringer.tick(now, offset);
    light.tick(now, ringer.ring());
    display.light(light.lit())?;
    if light.lit() {
        let frame = render(display.size(), &ringer.face(now, offset, low_battery));
        display.show(&frame)?;
    }
    Ok(now)
}

pub fn listen(
    input: &mut impl Input,
    seconds: u64,
    clock: &impl Clock,
    offset: UtcOffset,
    size: Size,
    ringer: &mut Ringer,
    light: &mut Light,
) -> Result<Option<InputEvent>, HardwareError> {
    let Some(event) = input.wait(seconds)? else {
        return Ok(None);
    };
    let now = clock.now()?;
    let ring = ringer.ring();
    if light.lit() {
        ringer.heard(event, size, now, offset);
    }
    light.heard(event, ring, now);
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::VecDeque;

    use super::*;
    use crate::face::Frame;
    use crate::light::LIT_SECONDS;
    use crate::time::TimeError;

    const PANEL: Size = Size {
        width: 1080,
        height: 2280,
    };
    const HOUR_UP: InputEvent = InputEvent::Touch { x: 400, y: 1600 };
    const HOUR_DOWN: InputEvent = InputEvent::Touch { x: 400, y: 1700 };
    const MINUTE_UP: InputEvent = InputEvent::Touch { x: 700, y: 1600 };
    const MINUTE_DOWN: InputEvent = InputEvent::Touch { x: 700, y: 1700 };
    const ELSEWHERE: InputEvent = InputEvent::Touch { x: 509, y: 1100 };
    const SNOOZE: InputEvent = InputEvent::Press(Key::VolumeDown);
    const DISMISS: InputEvent = InputEvent::Press(Key::Power);

    struct FakeClock(Cell<i64>);

    impl FakeClock {
        fn at(hour: i64, minute: i64, second: i64) -> Self {
            Self(Cell::new(
                20_768 * 86_400 + hour * 3_600 + minute * 60 + second,
            ))
        }

        fn pass(&self, seconds: i64) {
            self.0.set(self.0.get() + seconds);
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Result<UnixSeconds, HardwareError> {
            Ok(UnixSeconds(self.0.get()))
        }
    }

    #[derive(Default)]
    struct FakeInput {
        events: VecDeque<Result<InputEvent, HardwareError>>,
        waited: Vec<u64>,
    }

    impl FakeInput {
        fn with(events: &[InputEvent]) -> Self {
            Self {
                events: events.iter().copied().map(Ok).collect(),
                waited: Vec::new(),
            }
        }
    }

    impl Input for FakeInput {
        fn wait(&mut self, seconds: u64) -> Result<Option<InputEvent>, HardwareError> {
            self.waited.push(seconds);
            self.events.pop_front().transpose()
        }
    }

    #[derive(Default)]
    struct FakeDisplay {
        shown: Vec<Frame>,
        lit: Vec<bool>,
    }

    impl Display for FakeDisplay {
        fn size(&self) -> Size {
            PANEL
        }

        fn show(&mut self, frame: &Frame) -> Result<(), HardwareError> {
            self.shown.push(frame.clone());
            Ok(())
        }

        fn light(&mut self, on: bool) -> Result<(), HardwareError> {
            self.lit.push(on);
            Ok(())
        }
    }

    fn drain(
        input: &mut FakeInput,
        clock: &FakeClock,
        ringer: &mut Ringer,
    ) -> Result<(), HardwareError> {
        let mut light = always();
        while listen(input, 60, clock, UtcOffset::UTC, PANEL, ringer, &mut light)?.is_some() {}
        Ok(())
    }

    fn always() -> Light {
        Light::lit_at(UnixSeconds(0), i64::MAX / 2)
    }

    fn ringer(hour: u8, minute: u8) -> Result<Ringer, TimeError> {
        Ok(Ringer::new(TimeOfDay::new(hour, minute)?))
    }

    #[test]
    fn an_alarm_set_by_touch_rings_is_snoozed_rings_again_and_is_dismissed()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 58, 20);
        let mut ringer = ringer(7, 59)?;
        let mut input = FakeInput::with(&[HOUR_DOWN, MINUTE_UP, MINUTE_UP, MINUTE_DOWN]);
        drain(&mut input, &clock, &mut ringer)?;
        assert_eq!(ringer.alarm(), TimeOfDay::new(6, 0)?);
        assert!(!ringer.armed());
        let mut input = FakeInput::with(&[ELSEWHERE, ELSEWHERE, ELSEWHERE, HOUR_UP]);
        drain(&mut input, &clock, &mut ringer)?;
        assert_eq!(ringer.alarm(), TimeOfDay::new(7, 0)?);
        assert!(ringer.armed());
        assert_eq!(input.waited, [60; 5]);

        let mut display = FakeDisplay::default();
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            false,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            false,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert!(ringer.face(clock.now()?, UtcOffset::UTC, false).ringing);

        clock.pass(15);
        drain(&mut FakeInput::with(&[SNOOZE]), &clock, &mut ringer)?;
        let snoozed_until = UnixSeconds(20_768 * 86_400 + 7 * 3_600 + 9 * 60);
        assert_eq!(ringer.ring(), Ring::Snoozed(snoozed_until));
        clock.pass(8 * 60 + 24);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            false,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Snoozed(snoozed_until));
        clock.pass(1);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            false,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);

        drain(&mut FakeInput::with(&[DISMISS]), &clock, &mut ringer)?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert!(ringer.armed());
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            false,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert_eq!(display.shown.len(), 5);
        Ok(())
    }

    #[test]
    fn the_alarm_rings_again_the_next_day() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[ELSEWHERE]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(&mut FakeInput::with(&[DISMISS]), &clock, &mut ringer)?;
        clock.pass(86_400 - 60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Ringing);
        Ok(())
    }

    #[test]
    fn a_disarmed_alarm_never_rings() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        clock.pass(120);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert!(!ringer.armed());
        Ok(())
    }

    #[test]
    fn a_tap_elsewhere_disarms_a_snoozed_alarm() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[ELSEWHERE]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(
            &mut FakeInput::with(&[SNOOZE, ELSEWHERE]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert!(!ringer.armed());
        clock.pass(9 * 60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        Ok(())
    }

    #[test]
    fn while_ringing_touch_does_nothing_and_outside_ringing_keys_do_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(
            &mut FakeInput::with(&[SNOOZE, ELSEWHERE, DISMISS]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert!(ringer.armed());
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(
            &mut FakeInput::with(&[ELSEWHERE, HOUR_UP]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert_eq!(ringer.alarm(), TimeOfDay::new(7, 0)?);
        Ok(())
    }

    #[test]
    fn a_failing_input_is_returned_and_changes_nothing() -> Result<(), TimeError> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut input = FakeInput::default();
        input.events.push_back(Err(HardwareError::Absent("touch")));
        assert_eq!(
            listen(
                &mut input,
                30,
                &clock,
                UtcOffset::UTC,
                PANEL,
                &mut ringer,
                &mut always()
            ),
            Err(HardwareError::Absent("touch"))
        );
        assert_eq!(input.waited, [30]);
        assert_eq!(ringer, Ringer::new(TimeOfDay::new(7, 0)?));
        Ok(())
    }

    #[test]
    fn the_alarm_rings_at_local_time() -> Result<(), Box<dyn std::error::Error>> {
        let berlin_winter = UtcOffset::from_seconds(3_600)?;
        let clock = FakeClock::at(5, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut input = FakeInput::with(&[ELSEWHERE]);
        listen(
            &mut input,
            60,
            &clock,
            berlin_winter,
            PANEL,
            &mut ringer,
            &mut always(),
        )?;
        clock.pass(60);
        ringer.tick(clock.now()?, berlin_winter);
        assert_eq!(ringer.ring(), Ring::Ringing);
        Ok(())
    }

    #[test]
    fn a_dark_panel_draws_nothing_until_a_ringing_alarm_lights_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[ELSEWHERE]), &clock, &mut ringer)?;
        let mut light = Light::dark(LIT_SECONDS);
        let mut display = FakeDisplay::default();
        clock.pass(30);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            false,
            &mut display,
        )?;
        assert_eq!(display.lit, [false]);
        assert_eq!(display.shown, Vec::<Frame>::new());
        clock.pass(30);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            false,
            &mut display,
        )?;
        assert_eq!(display.lit, [false, true]);
        assert_eq!(display.shown.len(), 1);
        assert!(ringer.face(clock.now()?, UtcOffset::UTC, false).ringing);
        Ok(())
    }

    #[test]
    fn a_press_on_a_dark_panel_lights_it_and_leaves_a_snooze_alone()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[ELSEWHERE]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(&mut FakeInput::with(&[SNOOZE]), &clock, &mut ringer)?;
        let snoozed = ringer.ring();
        let mut light = Light::dark(LIT_SECONDS);
        let mut input = FakeInput::with(&[DISMISS]);
        listen(
            &mut input,
            60,
            &clock,
            UtcOffset::UTC,
            PANEL,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.ring(), snoozed);
        assert!(light.lit());
        let mut display = FakeDisplay::default();
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            false,
            &mut display,
        )?;
        assert_eq!(display.lit, [true]);
        assert_eq!(display.shown.len(), 1);
        Ok(())
    }

    #[test]
    fn a_touch_on_a_dark_panel_steps_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = Light::dark(LIT_SECONDS);
        let mut input = FakeInput::with(&[HOUR_UP, ELSEWHERE]);
        while listen(
            &mut input,
            60,
            &clock,
            UtcOffset::UTC,
            PANEL,
            &mut ringer,
            &mut light,
        )?
        .is_some()
        {}
        assert_eq!(ringer, Ringer::new(TimeOfDay::new(7, 0)?));
        assert!(!light.lit());
        Ok(())
    }

    #[test]
    fn a_press_on_a_lit_quiet_panel_turns_it_off() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = Light::lit_at(clock.now()?, LIT_SECONDS);
        let mut input = FakeInput::with(&[DISMISS]);
        listen(
            &mut input,
            60,
            &clock,
            UtcOffset::UTC,
            PANEL,
            &mut ringer,
            &mut light,
        )?;
        let mut display = FakeDisplay::default();
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            false,
            &mut display,
        )?;
        assert_eq!(display.lit, [false]);
        assert_eq!(display.shown, Vec::<Frame>::new());
        Ok(())
    }
}
