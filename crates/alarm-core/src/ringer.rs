use crate::alarm::{Alarm, Weekdays};
use crate::battery::Gauge;
use crate::face::{Face, Field, Row, SAVED_ALARMS, Screen, Size, Target, pitch, render, target};
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
struct Grab {
    field: Field,
    row: Row,
    from: usize,
    picked: TimeOfDay,
    moved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ringer {
    alarms: Vec<TimeOfDay>,
    active: Option<usize>,
    next: Option<UnixSeconds>,
    ring: Ring,
    picked: TimeOfDay,
    grab: Option<Grab>,
    marked: Option<usize>,
    power_off: bool,
}

fn stepped(time: TimeOfDay, field: Field, steps: i64) -> TimeOfDay {
    let (modulo, step): (i64, fn(TimeOfDay, i8) -> TimeOfDay) = match field {
        Field::Hour => (24, TimeOfDay::stepped_hour),
        Field::Minute => (60, TimeOfDay::stepped_minute),
    };
    i8::try_from(steps.rem_euclid(modulo)).map_or(time, |steps| step(time, steps))
}

fn after(alarm: TimeOfDay, now: UnixSeconds, offset: UtcOffset) -> UnixSeconds {
    Alarm {
        time: alarm,
        repeat: Weekdays::EVERY_DAY,
    }
    .next_after(now, offset)
}

impl Ringer {
    #[must_use]
    pub fn new(alarm: TimeOfDay) -> Self {
        Self::listed(vec![alarm], alarm)
    }

    #[must_use]
    pub fn listed(mut alarms: Vec<TimeOfDay>, picked: TimeOfDay) -> Self {
        alarms.sort_unstable();
        alarms.dedup();
        alarms.truncate(SAVED_ALARMS);
        Self {
            alarms,
            active: None,
            next: None,
            ring: Ring::Quiet,
            picked,
            grab: None,
            marked: None,
            power_off: true,
        }
    }

    #[must_use]
    pub fn alarms(&self) -> &[TimeOfDay] {
        &self.alarms
    }

    #[must_use]
    pub fn active(&self) -> Option<TimeOfDay> {
        self.active
            .and_then(|index| self.alarms.get(index).copied())
    }

    #[must_use]
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    #[must_use]
    pub fn picked(&self) -> TimeOfDay {
        self.picked
    }

    #[must_use]
    pub fn power_off(&self) -> bool {
        self.power_off
    }

    pub fn set_power_off(&mut self, power_off: bool) {
        self.power_off = power_off;
    }

    #[must_use]
    pub fn armed(&self) -> bool {
        self.next.is_some()
    }

    #[must_use]
    pub fn next(&self) -> Option<UnixSeconds> {
        self.next
    }

    #[must_use]
    pub fn ring(&self) -> Ring {
        self.ring
    }

    pub fn activate(&mut self, index: usize, now: UnixSeconds, offset: UtcOffset) {
        if let Some(&alarm) = self.alarms.get(index) {
            self.active = Some(index);
            self.next = Some(after(alarm, now, offset));
            self.ring = Ring::Quiet;
        }
    }

    fn disarm(&mut self) {
        self.active = None;
        self.next = None;
        self.ring = Ring::Quiet;
    }

    fn add(&mut self) {
        if self.alarms.len() >= SAVED_ALARMS || self.alarms.contains(&self.picked) {
            return;
        }
        let at = self.alarms.partition_point(|&alarm| alarm < self.picked);
        self.alarms.insert(at, self.picked);
        if let Some(active) = &mut self.active
            && *active >= at
        {
            *active += 1;
        }
    }

    fn remove(&mut self, index: usize) {
        if index >= self.alarms.len() {
            return;
        }
        match self.active {
            Some(active) if active == index => self.disarm(),
            Some(active) if active > index => self.active = Some(active - 1),
            _ => {}
        }
        self.alarms.remove(index);
    }

    pub fn tick(&mut self, now: UnixSeconds, offset: UtcOffset) {
        if let (Some(next), Some(alarm)) = (self.next, self.active())
            && now >= next
        {
            self.ring = Ring::Ringing;
            self.next = Some(after(alarm, now, offset));
        }
        if let Ring::Snoozed(until) = self.ring
            && now >= until
        {
            self.ring = Ring::Ringing;
        }
        if self.ring == Ring::Ringing {
            self.marked = None;
        }
    }

    pub fn pressed(&mut self, key: Key, now: UnixSeconds) {
        match (self.ring, key) {
            (Ring::Ringing, Key::VolumeUp | Key::VolumeDown) => {
                self.ring = Ring::Snoozed(UnixSeconds(start_of_minute(now).0 + SNOOZE_SECONDS));
            }
            (Ring::Ringing | Ring::Snoozed(_), Key::Power) => self.ring = Ring::Quiet,
            _ => {}
        }
    }

    pub fn darkened(&mut self) {
        self.grab = None;
        self.marked = None;
    }

    pub fn touched(&mut self, target: Target, y: usize, now: UnixSeconds, offset: UtcOffset) {
        let marked = self.marked.take();
        self.grab = None;
        match target {
            Target::Wheel(field, row) => {
                self.grab = Some(Grab {
                    field,
                    row,
                    from: y,
                    picked: self.picked,
                    moved: false,
                });
            }
            Target::Add => self.add(),
            Target::Entry(index) if self.active == Some(index) => self.disarm(),
            Target::Entry(index) => self.activate(index, now, offset),
            Target::Remove(index) if marked == Some(index) => self.remove(index),
            Target::Remove(index) if index < self.alarms.len() => self.marked = Some(index),
            Target::PowerOff => self.power_off = !self.power_off,
            Target::Remove(_) | Target::Tab(_) | Target::Blank => {}
        }
    }

    pub fn dragged(&mut self, y: usize, pitch: usize) {
        let Some(grab) = &mut self.grab else {
            return;
        };
        let pitch = pitch.max(1);
        let travel = grab.from.abs_diff(y);
        grab.moved |= 2 * travel >= pitch;
        if !grab.moved {
            return;
        }
        let steps = i64::try_from(travel / pitch).unwrap_or(0);
        let steps = if y > grab.from { -steps } else { steps };
        self.picked = stepped(grab.picked, grab.field, steps);
    }

    pub fn lifted(&mut self) {
        if let Some(grab) = self.grab.take()
            && !grab.moved
        {
            self.picked = stepped(self.picked, grab.field, i64::from(grab.row.step()));
        }
    }

    #[must_use]
    pub fn face(
        &self,
        now: UnixSeconds,
        offset: UtcOffset,
        battery: Gauge,
        screen: Screen,
    ) -> Face {
        Face {
            time: TimeOfDay::at(now, offset),
            alarms: self.alarms.clone(),
            active: self.active,
            marked: self.marked,
            picked: self.picked,
            ringing: self.ring == Ring::Ringing,
            battery,
            screen,
            power_off: self.power_off,
        }
    }
}

pub fn show(
    clock: &impl Clock,
    offset: UtcOffset,
    ringer: &mut Ringer,
    light: &mut Light,
    battery: Gauge,
    display: &mut impl Display,
) -> Result<UnixSeconds, HardwareError> {
    let now = clock.now()?;
    ringer.tick(now, offset);
    light.tick(now, ringer.ring());
    if !light.lit() {
        ringer.darkened();
    }
    display.light(light.lit())?;
    if light.lit() {
        let face = ringer.face(now, offset, battery, light.screen());
        let frame = render(display.size(), &face);
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
        match event {
            InputEvent::Press(key) => ringer.pressed(key, now),
            _ if ring == Ring::Ringing => {}
            InputEvent::Touch { x, y } => {
                let touched = target(size, light.screen(), x, y);
                if let Target::Tab(screen) = touched {
                    light.open(screen);
                }
                ringer.touched(touched, y, now, offset);
            }
            InputEvent::Move { y, .. } => ringer.dragged(y, pitch(size)),
            InputEvent::Lift => ringer.lifted(),
        }
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
    const PITCH: usize = 46;
    const HOUR_ABOVE: [InputEvent; 2] = tap(300, 800);
    const HOUR_BELOW: [InputEvent; 2] = tap(300, 1200);
    const MINUTE_ABOVE: [InputEvent; 2] = tap(600, 800);
    const MINUTE_BELOW: [InputEvent; 2] = tap(600, 1200);
    const ADD: [InputEvent; 2] = tap(900, 1000);
    const FIRST: [InputEvent; 2] = tap(300, 1400);
    const SECOND: [InputEvent; 2] = tap(300, 1600);
    const REMOVE_FIRST: [InputEvent; 2] = tap(900, 1400);
    const REMOVE_SECOND: [InputEvent; 2] = tap(900, 1600);
    const BLANK: [InputEvent; 2] = tap(540, 400);
    const POWER_OFF: [InputEvent; 2] = tap(540, 800);
    const ALARM_TAB: [InputEvent; 2] = tap(180, 2200);
    const TIMER_TAB: [InputEvent; 2] = tap(540, 2200);
    const SETTINGS_TAB: [InputEvent; 2] = tap(900, 2200);
    const SNOOZE: [InputEvent; 1] = [InputEvent::Press(Key::VolumeDown)];
    const DISMISS: [InputEvent; 1] = [InputEvent::Press(Key::Power)];

    const fn tap(x: usize, y: usize) -> [InputEvent; 2] {
        [InputEvent::Touch { x, y }, InputEvent::Lift]
    }

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
        fn with(gestures: &[&[InputEvent]]) -> Self {
            Self {
                events: gestures.concat().into_iter().map(Ok).collect(),
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
        drain_lit(input, clock, ringer, &mut always())
    }

    fn drain_lit(
        input: &mut FakeInput,
        clock: &FakeClock,
        ringer: &mut Ringer,
        light: &mut Light,
    ) -> Result<(), HardwareError> {
        while listen(input, 60, clock, UtcOffset::UTC, PANEL, ringer, light)?.is_some() {}
        Ok(())
    }

    fn always() -> Light {
        Light::lit_at(UnixSeconds(0), i64::MAX / 2)
    }

    fn ringer(hour: u8, minute: u8) -> Result<Ringer, TimeError> {
        Ok(Ringer::new(TimeOfDay::new(hour, minute)?))
    }

    fn times(times: &[(u8, u8)]) -> Result<Vec<TimeOfDay>, TimeError> {
        times
            .iter()
            .map(|&(hour, minute)| TimeOfDay::new(hour, minute))
            .collect()
    }

    #[test]
    fn an_activated_alarm_is_armed_for_its_next_time_and_a_new_list_is_not()
    -> Result<(), Box<dyn std::error::Error>> {
        let now = FakeClock::at(7, 30, 0).now()?;
        let mut ringer = Ringer::listed(times(&[(7, 0), (8, 0)])?, TimeOfDay::new(7, 0)?);
        assert!(!ringer.armed());
        assert_eq!(ringer.active(), None);
        ringer.activate(0, now, UtcOffset::UTC);
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        assert_eq!(ringer.next(), Some(FakeClock::at(31, 0, 0).now()?));
        assert_eq!(ringer.ring(), Ring::Quiet);
        ringer.activate(2, now, UtcOffset::UTC);
        assert_eq!(ringer.active_index(), Some(0));
        Ok(())
    }

    #[test]
    fn a_listed_ringer_is_sorted_without_duplicates_and_capped() -> Result<(), TimeError> {
        let ringer = Ringer::listed(
            times(&[(9, 0), (6, 0), (9, 0), (7, 0), (5, 0), (8, 0)])?,
            TimeOfDay::new(6, 0)?,
        );
        assert_eq!(ringer.alarms(), times(&[(5, 0), (6, 0), (7, 0), (8, 0)])?);
        assert_eq!(ringer.picked(), TimeOfDay::new(6, 0)?);
        Ok(())
    }

    #[test]
    fn an_alarm_set_on_the_slider_rings_is_snoozed_rings_again_and_is_dismissed()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 58, 20);
        let mut ringer = Ringer::listed(Vec::new(), TimeOfDay::new(7, 59)?);
        let mut input =
            FakeInput::with(&[&HOUR_ABOVE, &MINUTE_BELOW, &MINUTE_BELOW, &MINUTE_ABOVE]);
        drain(&mut input, &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(6, 0)?);
        assert_eq!(ringer.alarms(), []);
        assert!(!ringer.armed());
        let mut input = FakeInput::with(&[&HOUR_BELOW, &ADD, &FIRST, &FIRST, &FIRST]);
        drain(&mut input, &clock, &mut ringer)?;
        assert_eq!(ringer.alarms(), times(&[(7, 0)])?);
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        assert!(ringer.armed());
        assert_eq!(input.waited, [60; 11]);

        let mut display = FakeDisplay::default();
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert!(
            ringer
                .face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm)
                .ringing
        );

        clock.pass(15);
        drain(&mut FakeInput::with(&[&SNOOZE]), &clock, &mut ringer)?;
        let snoozed_until = UnixSeconds(20_768 * 86_400 + 7 * 3_600 + 9 * 60);
        assert_eq!(ringer.ring(), Ring::Snoozed(snoozed_until));
        clock.pass(8 * 60 + 24);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Snoozed(snoozed_until));
        clock.pass(1);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);

        drain(&mut FakeInput::with(&[&DISMISS]), &clock, &mut ringer)?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert!(ringer.armed());
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut always(),
            Gauge::UNREAD,
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
        assert_eq!(ringer.next(), None);
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        assert_eq!(ringer.next(), Some(UnixSeconds(clock.now()?.0 + 60)));
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.next(), Some(UnixSeconds(clock.now()?.0 + 86_400)));
        drain(&mut FakeInput::with(&[&DISMISS]), &clock, &mut ringer)?;
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
    fn a_tap_on_the_active_alarm_disarms_a_snooze() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(
            &mut FakeInput::with(&[&SNOOZE, &FIRST]),
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
            &mut FakeInput::with(&[&SNOOZE, &FIRST, &DISMISS]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert!(ringer.armed());
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(
            &mut FakeInput::with(&[&FIRST, &HOUR_ABOVE, &REMOVE_FIRST]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert_eq!(ringer.picked(), TimeOfDay::new(7, 0)?);
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
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
        let mut input = FakeInput::with(&[&FIRST]);
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
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let mut light = Light::dark(LIT_SECONDS);
        let mut display = FakeDisplay::default();
        clock.pass(30);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            Gauge::UNREAD,
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
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(display.lit, [false, true]);
        assert_eq!(display.shown.len(), 1);
        assert!(
            ringer
                .face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm)
                .ringing
        );
        Ok(())
    }

    #[test]
    fn a_press_on_a_dark_panel_lights_it_and_leaves_a_snooze_alone()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(&mut FakeInput::with(&[&SNOOZE]), &clock, &mut ringer)?;
        let snoozed = ringer.ring();
        let mut light = Light::dark(LIT_SECONDS);
        let mut input = FakeInput::with(&[&DISMISS]);
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
            Gauge::UNREAD,
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
        let mut input = FakeInput::with(&[&HOUR_ABOVE, &FIRST]);
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
        let mut input = FakeInput::with(&[&DISMISS]);
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
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(display.lit, [false]);
        assert_eq!(display.shown, Vec::<Frame>::new());
        Ok(())
    }

    #[test]
    fn a_tab_opens_its_screen_and_leaves_the_alarm_as_it_was()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&TIMER_TAB, &SETTINGS_TAB]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(light.screen(), Screen::Settings);
        assert_eq!(ringer, Ringer::new(TimeOfDay::new(7, 0)?));
        drain_lit(
            &mut FakeInput::with(&[&ALARM_TAB, &FIRST, &TIMER_TAB, &ALARM_TAB]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(light.screen(), Screen::Alarm);
        assert!(ringer.armed());
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        Ok(())
    }

    #[test]
    fn a_tap_on_an_empty_screen_changes_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let armed = ringer.clone();
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&TIMER_TAB, &FIRST, &HOUR_ABOVE, &MINUTE_BELOW, &ADD]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer, armed);
        Ok(())
    }

    #[test]
    fn a_tap_on_the_toggle_turns_the_power_off_off_and_on_again()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        assert!(ringer.power_off());
        let armed = ringer.clone();
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &POWER_OFF]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert!(!ringer.power_off());
        assert_eq!(ringer.alarms(), armed.alarms());
        assert_eq!(ringer.next(), armed.next());
        drain_lit(
            &mut FakeInput::with(&[&POWER_OFF]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer, armed);
        Ok(())
    }

    #[test]
    fn a_tap_off_the_toggle_on_the_settings_screen_changes_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let armed = ringer.clone();
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &FIRST, &HOUR_BELOW, &MINUTE_BELOW, &ADD]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(light.screen(), Screen::Settings);
        assert_eq!(ringer, armed);
        Ok(())
    }

    #[test]
    fn the_face_carries_the_toggle() -> Result<(), Box<dyn std::error::Error>> {
        let mut ringer = ringer(7, 0)?;
        let now = FakeClock::at(6, 59, 0).now()?;
        assert!(
            ringer
                .face(now, UtcOffset::UTC, Gauge::UNREAD, Screen::Settings)
                .power_off
        );
        ringer.set_power_off(false);
        assert!(
            !ringer
                .face(now, UtcOffset::UTC, Gauge::UNREAD, Screen::Settings)
                .power_off
        );
        Ok(())
    }

    #[test]
    fn the_open_screen_is_drawn_until_the_alarm_rings_over_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&TIMER_TAB]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let mut display = FakeDisplay::default();
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            Gauge::UNREAD,
            &mut display,
        )?;
        let timer = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
        assert_eq!(display.shown, [render(PANEL, &timer)]);
        clock.pass(60);
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(light.screen(), Screen::Alarm);
        assert_eq!(
            display.shown.last(),
            Some(&render(
                PANEL,
                &ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm)
            ))
        );
        Ok(())
    }

    fn drag(x: usize, from: usize, to: &[usize]) -> Vec<InputEvent> {
        [InputEvent::Touch { x, y: from }]
            .into_iter()
            .chain(to.iter().map(|&y| InputEvent::Move { x, y }))
            .chain([InputEvent::Lift])
            .collect()
    }

    #[test]
    fn dragging_a_wheel_up_counts_forward_one_step_per_pitch()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 0, 0);
        let mut ringer = ringer(7, 0)?;
        let gesture = drag(300, 1000, &[990, 1000 - 2 * PITCH, 1000 - 3 * PITCH - 1]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(10, 0)?);
        let gesture = drag(600, 1000, &[1000 + 2 * PITCH, 1000 + PITCH]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(10, 59)?);
        assert_eq!(ringer.alarms(), times(&[(7, 0)])?);
        assert!(!ringer.armed());
        Ok(())
    }

    #[test]
    fn a_drag_wraps_round_the_wheel_without_carrying() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 0, 0);
        let mut ringer = Ringer::listed(Vec::new(), TimeOfDay::new(23, 58)?);
        let gesture = drag(600, 800, &[800 - 3 * PITCH]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(23, 1)?);
        let gesture = drag(300, 800, &[800 - PITCH]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(0, 1)?);
        Ok(())
    }

    #[test]
    fn a_jitter_shorter_than_half_a_pitch_is_still_a_tap() -> Result<(), Box<dyn std::error::Error>>
    {
        let clock = FakeClock::at(6, 0, 0);
        let mut ringer = ringer(7, 0)?;
        let gesture = drag(300, 1200, &[1210, 1200 + PITCH / 2 - 1]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(8, 0)?);
        let gesture = drag(300, 1000, &[1000 + PITCH / 2]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(8, 0)?);
        Ok(())
    }

    #[test]
    fn a_move_with_nothing_grabbed_changes_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 0, 0);
        let mut ringer = ringer(7, 0)?;
        let gesture = drag(300, 1400, &[1000, 600]);
        drain(&mut FakeInput::with(&[&gesture]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(7, 0)?);
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        let moved = [InputEvent::Move { x: 300, y: 600 }, InputEvent::Lift];
        drain(&mut FakeInput::with(&[&moved]), &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(7, 0)?);
        Ok(())
    }

    #[test]
    fn an_added_alarm_goes_in_order_and_the_active_one_stays_active()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(5, 0, 0);
        let mut ringer = ringer(7, 0)?;
        drain(
            &mut FakeInput::with(&[&FIRST, &HOUR_ABOVE, &ADD, &HOUR_BELOW, &HOUR_BELOW, &ADD]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.alarms(), times(&[(6, 0), (7, 0), (8, 0)])?);
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        assert_eq!(ringer.next(), Some(FakeClock::at(7, 0, 0).now()?));
        Ok(())
    }

    #[test]
    fn a_duplicate_or_a_fifth_alarm_is_not_added() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(5, 0, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&ADD]), &clock, &mut ringer)?;
        assert_eq!(ringer.alarms(), times(&[(7, 0)])?);
        let mut ringer = Ringer::listed(
            times(&[(5, 0), (6, 0), (8, 0), (9, 0)])?,
            TimeOfDay::new(7, 0)?,
        );
        drain(&mut FakeInput::with(&[&ADD]), &clock, &mut ringer)?;
        assert_eq!(ringer.alarms().len(), SAVED_ALARMS);
        assert!(!ringer.alarms().contains(&TimeOfDay::new(7, 0)?));
        Ok(())
    }

    #[test]
    fn a_tap_on_another_entry_makes_it_the_active_alarm() -> Result<(), Box<dyn std::error::Error>>
    {
        let clock = FakeClock::at(5, 0, 0);
        let mut ringer = Ringer::listed(times(&[(6, 0), (7, 0)])?, TimeOfDay::new(6, 0)?);
        drain(
            &mut FakeInput::with(&[&FIRST, &SECOND]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        assert_eq!(ringer.next(), Some(FakeClock::at(7, 0, 0).now()?));
        drain(&mut FakeInput::with(&[&SECOND]), &clock, &mut ringer)?;
        assert_eq!(ringer.active(), None);
        assert!(!ringer.armed());
        Ok(())
    }

    #[test]
    fn removing_the_active_alarm_disarms_and_removing_another_keeps_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(5, 0, 0);
        let mut ringer = Ringer::listed(times(&[(6, 0), (7, 0), (8, 0)])?, TimeOfDay::new(6, 0)?);
        drain(
            &mut FakeInput::with(&[&SECOND, &REMOVE_FIRST, &REMOVE_FIRST]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.alarms(), times(&[(7, 0), (8, 0)])?);
        assert_eq!(ringer.active(), Some(TimeOfDay::new(7, 0)?));
        assert!(ringer.armed());
        drain(
            &mut FakeInput::with(&[&REMOVE_FIRST, &REMOVE_FIRST]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.alarms(), times(&[(8, 0)])?);
        assert_eq!(ringer.active(), None);
        assert!(!ringer.armed());
        drain(
            &mut FakeInput::with(&[&REMOVE_SECOND, &REMOVE_SECOND]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.alarms(), times(&[(8, 0)])?);
        Ok(())
    }

    #[test]
    fn a_first_tap_on_a_cross_only_marks_it_and_any_other_touch_clears_the_mark()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(5, 0, 0);
        let mut ringer = Ringer::listed(times(&[(6, 0), (7, 0)])?, TimeOfDay::new(6, 0)?);
        drain(&mut FakeInput::with(&[&REMOVE_FIRST]), &clock, &mut ringer)?;
        assert_eq!(ringer.alarms(), times(&[(6, 0), (7, 0)])?);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm);
        assert_eq!(face.marked, Some(0));
        for other in [&BLANK, &REMOVE_SECOND, &HOUR_ABOVE] {
            drain(
                &mut FakeInput::with(&[&BLANK, &REMOVE_FIRST, other, &REMOVE_FIRST]),
                &clock,
                &mut ringer,
            )?;
            assert_eq!(ringer.alarms(), times(&[(6, 0), (7, 0)])?);
            drain(&mut FakeInput::with(&[&BLANK]), &clock, &mut ringer)?;
        }
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm);
        assert_eq!(face.marked, None);
        Ok(())
    }

    #[test]
    fn the_panel_going_off_or_the_alarm_ringing_clears_the_mark()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = Ringer::listed(times(&[(7, 0), (8, 0)])?, TimeOfDay::new(7, 0)?);
        let mut light = Light::lit_at(clock.now()?, LIT_SECONDS);
        drain_lit(
            &mut FakeInput::with(&[&REMOVE_FIRST]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        clock.pass(LIT_SECONDS);
        let mut display = FakeDisplay::default();
        show(
            &clock,
            UtcOffset::UTC,
            &mut ringer,
            &mut light,
            Gauge::UNREAD,
            &mut display,
        )?;
        assert_eq!(display.lit, [false]);
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&REMOVE_FIRST]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.alarms(), times(&[(7, 0), (8, 0)])?);
        drain_lit(
            &mut FakeInput::with(&[&FIRST, &REMOVE_SECOND]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Ringing);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm);
        assert_eq!(face.marked, None);
        Ok(())
    }
}
