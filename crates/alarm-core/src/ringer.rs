use crate::alarm::{Alarm, Weekdays};
use crate::battery::Gauge;
use crate::face::{
    Face, Field, Row, SAVED_ALARMS, SAVED_TIMERS, Screen, Size, Target, pitch, render, target,
};
use crate::hardware::{Clock, Display, HardwareError, Input};
use crate::input::{InputEvent, Key};
use crate::light::{LIT_SECONDS, Light, next_lit_seconds};
use crate::time::{Countdown, TimeOfDay, UnixSeconds, UtcOffset};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ring {
    Quiet,
    Ringing,
    Held(UnixSeconds),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Picked {
    Alarm(TimeOfDay),
    Timer(Countdown),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Grab {
    field: Field,
    row: Row,
    from: usize,
    picked: Picked,
    moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Run {
    length: Countdown,
    ends: UnixSeconds,
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
    lit_seconds: i64,
    timers: Vec<Countdown>,
    countdown: Countdown,
    running: Option<Run>,
    rung: Option<Countdown>,
}

fn stepped(picked: Picked, field: Field, steps: i64) -> Picked {
    let modulo = match (picked, field) {
        (Picked::Alarm(_), Field::Left) => 24,
        (Picked::Timer(_), Field::Left) => i64::from(Countdown::LONGEST.minutes()) + 1,
        (_, Field::Right) => 60,
    };
    let Ok(step) = i8::try_from(steps.rem_euclid(modulo)) else {
        return picked;
    };
    match (picked, field) {
        (Picked::Alarm(time), Field::Left) => Picked::Alarm(time.stepped_hour(step)),
        (Picked::Alarm(time), Field::Right) => Picked::Alarm(time.stepped_minute(step)),
        (Picked::Timer(timer), Field::Left) => Picked::Timer(timer.stepped_minutes(step)),
        (Picked::Timer(timer), Field::Right) => Picked::Timer(timer.stepped_seconds(step)),
    }
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
            lit_seconds: LIT_SECONDS,
            timers: Vec::new(),
            countdown: Countdown::default(),
            running: None,
            rung: None,
        }
    }

    pub fn set_timers(&mut self, mut timers: Vec<Countdown>) {
        timers.retain(|timer| !timer.is_zero());
        timers.sort_unstable();
        timers.dedup();
        timers.truncate(SAVED_TIMERS);
        self.countdown = timers.first().copied().unwrap_or_default();
        self.timers = timers;
        self.marked = None;
    }

    #[must_use]
    pub fn timers(&self) -> &[Countdown] {
        &self.timers
    }

    #[must_use]
    pub fn countdown(&self) -> Countdown {
        self.countdown
    }

    #[must_use]
    pub fn running_until(&self) -> Option<UnixSeconds> {
        self.running.map(|run| run.ends)
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
    pub fn lit_seconds(&self) -> i64 {
        self.lit_seconds
    }

    pub fn set_lit_seconds(&mut self, lit_seconds: i64) {
        self.lit_seconds = lit_seconds;
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
        if self.rung.is_some() {
            Ring::Ringing
        } else {
            self.ring
        }
    }

    #[must_use]
    pub fn held(&self) -> Ring {
        match (self.ring(), self.running) {
            (Ring::Quiet, Some(run)) => Ring::Held(run.ends),
            (ring, _) => ring,
        }
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

    fn add_timer(&mut self) {
        if self.timers.len() >= SAVED_TIMERS
            || self.countdown.is_zero()
            || self.timers.contains(&self.countdown)
        {
            return;
        }
        let at = self.timers.partition_point(|&timer| timer < self.countdown);
        self.timers.insert(at, self.countdown);
    }

    fn start(&mut self, now: UnixSeconds) {
        self.running = match self.running {
            Some(_) => None,
            None if self.countdown.is_zero() => None,
            None => Some(Run {
                length: self.countdown,
                ends: UnixSeconds(now.0.saturating_add(self.countdown.total())),
            }),
        };
    }

    fn picked_on(&self, screen: Screen) -> Picked {
        match screen {
            Screen::Timer => Picked::Timer(self.countdown),
            Screen::Alarm | Screen::Settings => Picked::Alarm(self.picked),
        }
    }

    fn pick(&mut self, picked: Picked) {
        match picked {
            Picked::Alarm(time) => self.picked = time,
            Picked::Timer(timer) => self.countdown = timer,
        }
    }

    pub fn tick(&mut self, now: UnixSeconds, offset: UtcOffset) {
        if let (Some(next), Some(alarm)) = (self.next, self.active())
            && now >= next
        {
            self.ring = Ring::Ringing;
            self.next = Some(after(alarm, now, offset));
        }
        if let Some(run) = self.running
            && now >= run.ends
        {
            self.running = None;
            self.rung = Some(run.length);
        }
        if self.ring() == Ring::Ringing {
            self.marked = None;
        }
    }

    pub fn pressed(&mut self, key: Key) {
        if key == Key::Power {
            self.ring = Ring::Quiet;
            self.rung = None;
        }
    }

    pub fn darkened(&mut self) {
        self.grab = None;
        self.marked = None;
    }

    fn touched_timer(&mut self, target: Target, marked: Option<usize>) {
        match target {
            Target::Add => self.add_timer(),
            Target::Entry(index) => {
                if let Some(&timer) = self.timers.get(index) {
                    self.countdown = timer;
                }
            }
            Target::Remove(index) if index < self.timers.len() && marked == Some(index) => {
                self.timers.remove(index);
            }
            Target::Remove(index) if index < self.timers.len() => self.marked = Some(index),
            _ => {}
        }
    }

    pub fn touched(
        &mut self,
        target: Target,
        screen: Screen,
        y: usize,
        now: UnixSeconds,
        offset: UtcOffset,
    ) {
        let marked = self.marked.take();
        self.grab = None;
        match target {
            Target::Wheel(field, row) => {
                self.grab = Some(Grab {
                    field,
                    row,
                    from: y,
                    picked: self.picked_on(screen),
                    moved: false,
                });
            }
            Target::Start => self.start(now),
            _ if screen == Screen::Timer => self.touched_timer(target, marked),
            Target::Add => self.add(),
            Target::Entry(index) if self.active == Some(index) => self.disarm(),
            Target::Entry(index) => self.activate(index, now, offset),
            Target::Remove(index) if marked == Some(index) => self.remove(index),
            Target::Remove(index) if index < self.alarms.len() => self.marked = Some(index),
            Target::PowerOff => self.power_off = !self.power_off,
            Target::LitTime => self.lit_seconds = next_lit_seconds(self.lit_seconds),
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
        let picked = stepped(grab.picked, grab.field, steps);
        self.pick(picked);
    }

    pub fn lifted(&mut self) {
        if let Some(grab) = self.grab.take()
            && !grab.moved
        {
            self.pick(stepped(grab.picked, grab.field, i64::from(grab.row.step())));
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
            ringing: self.ring() == Ring::Ringing,
            battery,
            screen,
            power_off: self.power_off,
            lit_seconds: self.lit_seconds,
            timers: self.timers.clone(),
            countdown: self.countdown,
            left: self
                .running
                .map(|run| Countdown::of(run.ends.0.saturating_sub(now.0))),
            rung: self.rung.filter(|_| self.ring != Ring::Ringing),
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
            InputEvent::Press(key) => ringer.pressed(key),
            _ if ring == Ring::Ringing => {}
            InputEvent::Touch { x, y } => {
                let touched = target(size, light.screen(), x, y);
                if let Target::Tab(screen) = touched {
                    light.open(screen);
                }
                ringer.touched(touched, light.screen(), y, now, offset);
            }
            InputEvent::Move { y, .. } => ringer.dragged(y, pitch(size)),
            InputEvent::Lift => ringer.lifted(),
        }
    }
    light.set_seconds(ringer.lit_seconds());
    light.heard(event, ring, now);
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::VecDeque;

    use super::*;
    use crate::face::Frame;
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
    const LIT_TIME: [InputEvent; 2] = tap(540, 1000);
    const ALARM_TAB: [InputEvent; 2] = tap(180, 2200);
    const TIMER_TAB: [InputEvent; 2] = tap(540, 2200);
    const SETTINGS_TAB: [InputEvent; 2] = tap(900, 2200);
    const START: [InputEvent; 2] = tap(540, 580);
    const VOLUME: [InputEvent; 2] = [
        InputEvent::Press(Key::VolumeUp),
        InputEvent::Press(Key::VolumeDown),
    ];
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

    fn countdowns(timers: &[(u8, u8)]) -> Result<Vec<Countdown>, TimeError> {
        timers
            .iter()
            .map(|&(minutes, seconds)| Countdown::new(minutes, seconds))
            .collect()
    }

    fn timed(timers: &[(u8, u8)]) -> Result<Ringer, TimeError> {
        let mut ringer = ringer(7, 0)?;
        ringer.set_timers(countdowns(timers)?);
        Ok(ringer)
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
    fn an_alarm_set_on_the_slider_rings_until_power_dismisses_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 58, 20);
        let mut ringer = Ringer::listed(Vec::new(), TimeOfDay::new(7, 59)?);
        let mut input =
            FakeInput::with(&[&HOUR_BELOW, &MINUTE_ABOVE, &MINUTE_ABOVE, &MINUTE_BELOW]);
        drain(&mut input, &clock, &mut ringer)?;
        assert_eq!(ringer.picked(), TimeOfDay::new(6, 0)?);
        assert_eq!(ringer.alarms(), []);
        assert!(!ringer.armed());
        let mut input = FakeInput::with(&[&HOUR_ABOVE, &ADD, &FIRST, &FIRST, &FIRST]);
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
        drain(&mut FakeInput::with(&[&VOLUME]), &clock, &mut ringer)?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        clock.pass(9 * 60);
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
        assert_eq!(display.shown.len(), 4);
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
    fn a_volume_key_leaves_a_ringing_alarm_ringing() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        drain(&mut FakeInput::with(&[&VOLUME]), &clock, &mut ringer)?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert!(ringer.armed());
        clock.pass(9 * 60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Ringing);
        drain(&mut FakeInput::with(&[&DISMISS]), &clock, &mut ringer)?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        Ok(())
    }

    #[test]
    fn while_ringing_touch_does_nothing_and_outside_ringing_keys_do_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(
            &mut FakeInput::with(&[&VOLUME, &FIRST, &DISMISS]),
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
    fn a_press_on_a_dark_panel_lights_it_and_leaves_a_ring_ringing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
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
        assert_eq!(ringer.ring(), Ring::Ringing);
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
    fn a_tap_on_the_timer_screen_leaves_the_alarms_as_they_were()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = timed(&[(5, 0)])?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let armed = ringer.clone();
        drain(
            &mut FakeInput::with(&[
                &TIMER_TAB,
                &FIRST,
                &HOUR_ABOVE,
                &MINUTE_BELOW,
                &ADD,
                &REMOVE_FIRST,
            ]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.alarms(), armed.alarms());
        assert_eq!(ringer.active(), armed.active());
        assert_eq!(ringer.picked(), armed.picked());
        assert_eq!(ringer.next(), armed.next());
        assert_ne!(ringer.timers(), armed.timers());
        Ok(())
    }

    #[test]
    fn a_set_of_timers_starts_on_its_shortest_without_zeros_duplicates_or_a_fifth()
    -> Result<(), TimeError> {
        let ringer = timed(&[(9, 0), (0, 0), (1, 30), (9, 0), (3, 0), (0, 5), (20, 0)])?;
        assert_eq!(
            ringer.timers(),
            countdowns(&[(0, 5), (1, 30), (3, 0), (9, 0)])?
        );
        assert_eq!(ringer.countdown(), Countdown::new(0, 5)?);
        assert_eq!(timed(&[])?.countdown(), Countdown::default());
        Ok(())
    }

    #[test]
    fn the_timer_wheels_step_minutes_and_seconds_and_wrap_within_each()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[])?;
        let mut light = always();
        let mut on = |gestures: &[&[InputEvent]], ringer: &mut Ringer| {
            drain_lit(&mut FakeInput::with(gestures), &clock, ringer, &mut light)
        };
        on(&[&TIMER_TAB, &HOUR_BELOW, &MINUTE_BELOW], &mut ringer)?;
        assert_eq!(ringer.countdown(), Countdown::LONGEST);
        on(&[&HOUR_ABOVE, &MINUTE_ABOVE, &MINUTE_ABOVE], &mut ringer)?;
        assert_eq!(ringer.countdown(), Countdown::new(0, 1)?);
        on(&[&drag(300, 1000, &[1000 - 3 * PITCH])], &mut ringer)?;
        assert_eq!(ringer.countdown(), Countdown::new(3, 1)?);
        Ok(())
    }

    #[test]
    fn a_timer_is_added_in_order_once_and_never_at_zero_or_past_four()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(5, 0)])?;
        drain(
            &mut FakeInput::with(&[
                &TIMER_TAB,
                &HOUR_BELOW,
                &HOUR_BELOW,
                &MINUTE_ABOVE,
                &ADD,
                &ADD,
            ]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.timers(), countdowns(&[(3, 1), (5, 0)])?);
        let mut zero = timed(&[])?;
        drain(&mut FakeInput::with(&[&TIMER_TAB, &ADD]), &clock, &mut zero)?;
        assert_eq!(zero.timers(), []);
        let mut full = timed(&[(1, 0), (2, 0), (3, 0), (4, 0)])?;
        drain(
            &mut FakeInput::with(&[&TIMER_TAB, &MINUTE_BELOW, &ADD]),
            &clock,
            &mut full,
        )?;
        assert_eq!(
            full.timers(),
            countdowns(&[(1, 0), (2, 0), (3, 0), (4, 0)])?
        );
        Ok(())
    }

    #[test]
    fn a_timer_cross_marks_on_the_first_tap_and_removes_on_the_second()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 0), (2, 0)])?;
        let mut light = always();
        let mut on = |gestures: &[&[InputEvent]], ringer: &mut Ringer| {
            drain_lit(&mut FakeInput::with(gestures), &clock, ringer, &mut light)
        };
        on(&[&TIMER_TAB, &REMOVE_SECOND], &mut ringer)?;
        assert_eq!(ringer.timers(), countdowns(&[(1, 0), (2, 0)])?);
        on(&[&REMOVE_SECOND], &mut ringer)?;
        assert_eq!(ringer.timers(), countdowns(&[(1, 0)])?);
        on(&[&REMOVE_FIRST, &BLANK, &REMOVE_FIRST], &mut ringer)?;
        assert_eq!(ringer.timers(), countdowns(&[(1, 0)])?);
        Ok(())
    }

    #[test]
    fn a_timer_started_from_its_entry_rings_at_its_end_until_power()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30), (5, 0)])?;
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&TIMER_TAB, &SECOND, &START]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let ends = UnixSeconds(clock.now()?.0 + 300);
        assert_eq!(ringer.running_until(), Some(ends));
        assert_eq!(ringer.held(), Ring::Held(ends));
        clock.pass(1);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
        assert_eq!(face.left, Some(Countdown::new(4, 59)?));
        clock.pass(298);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        clock.pass(1);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert_eq!(ringer.running_until(), None);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
        assert!(face.ringing);
        assert_eq!(face.rung, Some(Countdown::new(5, 0)?));
        drain_lit(
            &mut FakeInput::with(&[&VOLUME, &START]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert_eq!(ringer.running_until(), None);
        drain_lit(
            &mut FakeInput::with(&[&DISMISS]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert_eq!(ringer.held(), Ring::Quiet);
        Ok(())
    }

    #[test]
    fn start_stops_a_running_timer_and_does_nothing_at_zero()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30)])?;
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&TIMER_TAB, &START]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(
            ringer.running_until(),
            Some(UnixSeconds(clock.now()?.0 + 90))
        );
        drain_lit(
            &mut FakeInput::with(&[&START]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.running_until(), None);
        clock.pass(90);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        let mut zero = timed(&[])?;
        drain(
            &mut FakeInput::with(&[&TIMER_TAB, &START]),
            &clock,
            &mut zero,
        )?;
        assert_eq!(zero.running_until(), None);
        Ok(())
    }

    #[test]
    fn a_running_timer_survives_power_and_holds_the_phone_after_a_dismissed_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = timed(&[(5, 0)])?;
        drain(
            &mut FakeInput::with(&[&FIRST, &TIMER_TAB, &START, &DISMISS]),
            &clock,
            &mut ringer,
        )?;
        let ends = FakeClock::at(7, 4, 0).now()?;
        assert_eq!(ringer.running_until(), Some(ends));
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.held(), Ring::Ringing);
        ringer.pressed(Key::VolumeDown);
        assert_eq!(ringer.held(), Ring::Ringing);
        ringer.pressed(Key::Power);
        assert_eq!(ringer.ring(), Ring::Quiet);
        assert_eq!(ringer.held(), Ring::Held(ends));
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
    fn a_tap_on_the_lit_time_steps_it_through_the_choices_and_round()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        assert_eq!(ringer.lit_seconds(), LIT_SECONDS);
        let armed = ringer.clone();
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let mut seen = Vec::new();
        for _ in 0..5 {
            drain_lit(
                &mut FakeInput::with(&[&LIT_TIME]),
                &clock,
                &mut ringer,
                &mut light,
            )?;
            seen.push(ringer.lit_seconds());
        }
        assert_eq!(seen, [30, 60, 120, 300, 10]);
        assert_eq!(ringer, armed);
        Ok(())
    }

    #[test]
    fn the_panel_stays_lit_for_the_lit_time_from_the_tap_that_sets_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = Light::lit_at(clock.now()?, LIT_SECONDS);
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &LIT_TIME]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.lit_seconds(), 30);
        assert_eq!(light.seconds_left(clock.now()?), Some(30));
        clock.pass(29);
        light.tick(clock.now()?, ringer.ring());
        assert!(light.lit());
        clock.pass(1);
        light.tick(clock.now()?, ringer.ring());
        assert!(!light.lit());
        drain_lit(
            &mut FakeInput::with(&[&DISMISS]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(light.seconds_left(clock.now()?), Some(30));
        Ok(())
    }

    #[test]
    fn a_tap_off_the_rows_on_the_settings_screen_changes_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let armed = ringer.clone();
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &FIRST, &HOUR_BELOW, &MINUTE_BELOW, &BLANK]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(light.screen(), Screen::Settings);
        assert_eq!(ringer, armed);
        Ok(())
    }

    #[test]
    fn the_face_carries_the_toggle_and_the_lit_time() -> Result<(), Box<dyn std::error::Error>> {
        let mut ringer = ringer(7, 0)?;
        let now = FakeClock::at(6, 59, 0).now()?;
        let face = ringer.face(now, UtcOffset::UTC, Gauge::UNREAD, Screen::Settings);
        assert!(face.power_off);
        assert_eq!(face.lit_seconds, LIT_SECONDS);
        ringer.set_power_off(false);
        ringer.set_lit_seconds(120);
        let face = ringer.face(now, UtcOffset::UTC, Gauge::UNREAD, Screen::Settings);
        assert!(!face.power_off);
        assert_eq!(face.lit_seconds, 120);
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
    fn a_tap_on_the_up_arrow_counts_up_and_on_the_down_arrow_counts_down()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 0, 0);
        let mut alarm = Ringer::listed(Vec::new(), TimeOfDay::new(23, 59)?);
        drain(&mut FakeInput::with(&[&HOUR_ABOVE]), &clock, &mut alarm)?;
        assert_eq!(alarm.picked(), TimeOfDay::new(0, 59)?);
        drain(&mut FakeInput::with(&[&MINUTE_ABOVE]), &clock, &mut alarm)?;
        assert_eq!(alarm.picked(), TimeOfDay::new(0, 0)?);
        drain(&mut FakeInput::with(&[&HOUR_BELOW]), &clock, &mut alarm)?;
        assert_eq!(alarm.picked(), TimeOfDay::new(23, 0)?);
        drain(&mut FakeInput::with(&[&MINUTE_BELOW]), &clock, &mut alarm)?;
        assert_eq!(alarm.picked(), TimeOfDay::new(23, 59)?);

        let mut timer = timed(&[])?;
        let mut light = always();
        let mut on = |gestures: &[&[InputEvent]], ringer: &mut Ringer| {
            drain_lit(&mut FakeInput::with(gestures), &clock, ringer, &mut light)
        };
        on(&[&TIMER_TAB, &HOUR_ABOVE], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::new(1, 0)?);
        on(&[&MINUTE_ABOVE], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::new(1, 1)?);
        on(&[&HOUR_BELOW, &HOUR_BELOW], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::new(99, 1)?);
        on(&[&MINUTE_BELOW, &MINUTE_BELOW], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::LONGEST);
        Ok(())
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
        let gesture = drag(300, 800, &[810, 800 + PITCH / 2 - 1]);
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
