use crate::alarm::{Alarm, Weekdays};
use crate::battery::Gauge;
use crate::face::{
    Face, Field, Row, Running, SAVED_ALARMS, SAVED_TIMERS, Screen, Size, Stage, Target, pitch,
    render, target,
};
use crate::hardware::{Clock, Display, HardwareError, Input};
use crate::input::{InputEvent, Key};
use crate::light::{LIT_SECONDS, Light, next_lit_seconds};
use crate::sound::Volume;
use crate::time::{Countdown, TimeOfDay, UnixSeconds, UtcOffset};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ring {
    Quiet,
    Ringing,
    Held(UnixSeconds),
    Paused,
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
enum Count {
    Ends(UnixSeconds),
    Paused(Countdown),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Run {
    length: Countdown,
    count: Count,
}

impl Run {
    fn left(self, now: UnixSeconds) -> Countdown {
        match self.count {
            Count::Ends(ends) => Countdown::of(ends.0.saturating_sub(now.0)),
            Count::Paused(left) => left,
        }
    }

    fn ends(self) -> Option<UnixSeconds> {
        match self.count {
            Count::Ends(ends) => Some(ends),
            Count::Paused(_) => None,
        }
    }
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
    volume: Volume,
    recovery_marked: bool,
    recovery: bool,
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
            volume: Volume::LOUDEST,
            recovery_marked: false,
            recovery: false,
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
        self.running.and_then(Run::ends)
    }

    #[must_use]
    pub fn paused(&self) -> Option<Countdown> {
        match self.running?.count {
            Count::Paused(left) => Some(left),
            Count::Ends(_) => None,
        }
    }

    #[must_use]
    pub fn stage(&self) -> Stage {
        if self.running.is_some() {
            Stage::Running
        } else {
            Stage::List
        }
    }

    #[must_use]
    pub fn ringing_screen(&self) -> Screen {
        if self.rung.is_some() && self.ring != Ring::Ringing {
            Screen::Timer
        } else {
            Screen::Alarm
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
    pub fn lit_seconds(&self) -> i64 {
        self.lit_seconds
    }

    pub fn set_lit_seconds(&mut self, lit_seconds: i64) {
        self.lit_seconds = lit_seconds;
    }

    #[must_use]
    pub fn volume(&self) -> Volume {
        self.volume
    }

    pub fn set_volume(&mut self, volume: Volume) {
        self.volume = volume;
    }

    #[must_use]
    pub fn take_recovery(&mut self) -> bool {
        std::mem::take(&mut self.recovery)
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
        match (self.ring(), self.running.map(|run| run.count)) {
            (Ring::Quiet, Some(Count::Ends(ends))) => Ring::Held(ends),
            (Ring::Quiet, Some(Count::Paused(_))) => Ring::Paused,
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

    fn play(&mut self, index: usize, now: UnixSeconds) {
        if self.running.is_some() {
            return;
        }
        if let Some(&length) = self.timers.get(index) {
            self.running = Some(Run {
                length,
                count: Count::Ends(UnixSeconds(now.0.saturating_add(length.total()))),
            });
        }
    }

    fn pause(&mut self, now: UnixSeconds) {
        if let Some(run) = &mut self.running {
            run.count = match run.count {
                Count::Ends(ends) => Count::Paused(Countdown::of(ends.0.saturating_sub(now.0))),
                Count::Paused(left) => Count::Ends(UnixSeconds(now.0.saturating_add(left.total()))),
            };
        }
    }

    fn close(&mut self) {
        if self.paused().is_some() {
            self.running = None;
        }
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
            && run.ends().is_some_and(|ends| now >= ends)
        {
            self.running = None;
            self.rung = Some(run.length);
        }
        if self.ring() == Ring::Ringing {
            self.marked = None;
            self.recovery_marked = false;
        }
    }

    pub fn pressed(&mut self, key: Key) {
        match key {
            Key::Power => {
                self.ring = Ring::Quiet;
                self.rung = None;
            }
            Key::VolumeUp => self.volume = self.volume.up(),
            Key::VolumeDown => self.volume = self.volume.down(),
        }
    }

    pub fn darkened(&mut self) {
        self.grab = None;
        self.marked = None;
        self.recovery_marked = false;
    }

    fn touched_timer(&mut self, target: Target, marked: Option<usize>, now: UnixSeconds) {
        match target {
            Target::Add => self.add_timer(),
            Target::Play(index) => self.play(index, now),
            Target::Pause => self.pause(now),
            Target::Close => self.close(),
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
        let recovery_marked = std::mem::take(&mut self.recovery_marked);
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
            _ if screen == Screen::Timer => self.touched_timer(target, marked, now),
            Target::Add => self.add(),
            Target::Entry(index) if self.active == Some(index) => self.disarm(),
            Target::Entry(index) => self.activate(index, now, offset),
            Target::Remove(index) if marked == Some(index) => self.remove(index),
            Target::Remove(index) if index < self.alarms.len() => self.marked = Some(index),
            Target::PowerOff => self.power_off = !self.power_off,
            Target::LitTime => self.lit_seconds = next_lit_seconds(self.lit_seconds),
            Target::Recovery if recovery_marked => self.recovery = true,
            Target::Recovery => self.recovery_marked = true,
            Target::Remove(_)
            | Target::Play(_)
            | Target::Pause
            | Target::Close
            | Target::Tab(_)
            | Target::Blank => {}
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
            recovery_marked: self.recovery_marked,
            timers: self.timers.clone(),
            countdown: self.countdown,
            running: self.running.map(|run| Running {
                left: run.left(now),
                length: run.length,
                paused: run.ends().is_none(),
            }),
            rung: self.rung.filter(|_| self.ring != Ring::Ringing),
            volume: None,
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
    if ringer.ring() == Ring::Ringing {
        light.open(ringer.ringing_screen());
    }
    if !light.lit() {
        ringer.darkened();
    }
    display.light(light.lit())?;
    if light.lit() {
        let face = Face {
            volume: light.volume_shown().then_some(ringer.volume()),
            ..ringer.face(now, offset, battery, light.screen())
        };
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
                let touched = target(size, light.screen(), ringer.stage(), x, y);
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
    use crate::light::VOLUME_SECONDS;
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
    const RECOVERY: [InputEvent; 2] = tap(540, 1180);
    const ALARM_TAB: [InputEvent; 2] = tap(180, 2200);
    const TIMER_TAB: [InputEvent; 2] = tap(540, 2200);
    const SETTINGS_TAB: [InputEvent; 2] = tap(900, 2200);
    const TIMER_MINUTES_ABOVE: [InputEvent; 2] = tap(200, 220);
    const TIMER_MINUTES_BELOW: [InputEvent; 2] = tap(200, 600);
    const TIMER_SECONDS_ABOVE: [InputEvent; 2] = tap(450, 220);
    const TIMER_SECONDS_BELOW: [InputEvent; 2] = tap(450, 600);
    const TIMER_ADD: [InputEvent; 2] = tap(700, 400);
    const TIMER_FIRST: [InputEvent; 2] = tap(300, 820);
    const PLAY_FIRST: [InputEvent; 2] = tap(700, 820);
    const PLAY_SECOND: [InputEvent; 2] = tap(700, 1000);
    const TIMER_REMOVE_FIRST: [InputEvent; 2] = tap(930, 820);
    const TIMER_REMOVE_SECOND: [InputEvent; 2] = tap(930, 1000);
    const TIMER_BLANK: [InputEvent; 2] = tap(540, 1600);
    const PAUSE: [InputEvent; 2] = tap(540, 1667);
    const CLOSE: [InputEvent; 2] = tap(218, 1667);
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
    fn the_volume_keys_step_the_volume_and_stop_at_either_end()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        assert_eq!(ringer.volume(), Volume::LOUDEST);
        let down = [InputEvent::Press(Key::VolumeDown); 3];
        drain(&mut FakeInput::with(&[&down]), &clock, &mut ringer)?;
        assert_eq!(ringer.volume().level(), 12);
        drain(&mut FakeInput::with(&[&VOLUME]), &clock, &mut ringer)?;
        assert_eq!(ringer.volume().level(), 12);
        let up = [InputEvent::Press(Key::VolumeUp); 20];
        drain(&mut FakeInput::with(&[&up]), &clock, &mut ringer)?;
        assert_eq!(ringer.volume(), Volume::LOUDEST);
        let down = [InputEvent::Press(Key::VolumeDown); 20];
        drain(&mut FakeInput::with(&[&down]), &clock, &mut ringer)?;
        assert_eq!(ringer.volume(), Volume::QUIETEST);
        Ok(())
    }

    #[test]
    fn a_volume_key_on_a_dark_panel_changes_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = Light::dark(LIT_SECONDS);
        drain_lit(
            &mut FakeInput::with(&[&VOLUME, &[InputEvent::Press(Key::VolumeDown)]]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer, Ringer::new(TimeOfDay::new(7, 0)?));
        assert_eq!(light, Light::dark(LIT_SECONDS));
        Ok(())
    }

    #[test]
    fn a_volume_key_during_a_ring_steps_the_volume_and_leaves_it_ringing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        let mut light = Light::dark(LIT_SECONDS);
        light.tick(clock.now()?, ringer.ring());
        drain_lit(
            &mut FakeInput::with(&[&[InputEvent::Press(Key::VolumeDown)]]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert_eq!(ringer.volume(), Volume::LOUDEST.down());
        assert!(light.volume_shown());
        Ok(())
    }

    #[test]
    fn the_volume_shows_on_the_face_after_a_key_and_is_gone_three_seconds_later()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = Light::lit_at(clock.now()?, LIT_SECONDS);
        drain_lit(
            &mut FakeInput::with(&[&[InputEvent::Press(Key::VolumeDown)]]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let mut display = FakeDisplay::default();
        for _ in 0..2 {
            show(
                &clock,
                UtcOffset::UTC,
                &mut ringer,
                &mut light,
                Gauge::UNREAD,
                &mut display,
            )?;
            clock.pass(3);
        }
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Alarm);
        let shown = Face {
            volume: Some(Volume::LOUDEST.down()),
            ..face.clone()
        };
        assert_eq!(display.shown, [render(PANEL, &shown), render(PANEL, &face)]);
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
                &TIMER_FIRST,
                &TIMER_MINUTES_ABOVE,
                &TIMER_SECONDS_BELOW,
                &TIMER_ADD,
                &TIMER_REMOVE_FIRST,
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
        on(
            &[&TIMER_TAB, &TIMER_MINUTES_BELOW, &TIMER_SECONDS_BELOW],
            &mut ringer,
        )?;
        assert_eq!(ringer.countdown(), Countdown::LONGEST);
        on(
            &[
                &TIMER_MINUTES_ABOVE,
                &TIMER_SECONDS_ABOVE,
                &TIMER_SECONDS_ABOVE,
            ],
            &mut ringer,
        )?;
        assert_eq!(ringer.countdown(), Countdown::new(0, 1)?);
        on(&[&drag(200, 400, &[400 - 3 * PITCH])], &mut ringer)?;
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
                &TIMER_MINUTES_BELOW,
                &TIMER_MINUTES_BELOW,
                &TIMER_SECONDS_ABOVE,
                &TIMER_ADD,
                &TIMER_ADD,
            ]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.timers(), countdowns(&[(3, 1), (5, 0)])?);
        let mut zero = timed(&[])?;
        drain(
            &mut FakeInput::with(&[&TIMER_TAB, &TIMER_ADD]),
            &clock,
            &mut zero,
        )?;
        assert_eq!(zero.timers(), []);
        let mut full = timed(&[(1, 0), (2, 0), (3, 0), (4, 0)])?;
        drain(
            &mut FakeInput::with(&[&TIMER_TAB, &TIMER_SECONDS_BELOW, &TIMER_ADD]),
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
        on(&[&TIMER_TAB, &TIMER_REMOVE_SECOND], &mut ringer)?;
        assert_eq!(ringer.timers(), countdowns(&[(1, 0), (2, 0)])?);
        on(&[&TIMER_REMOVE_SECOND], &mut ringer)?;
        assert_eq!(ringer.timers(), countdowns(&[(1, 0)])?);
        on(
            &[&TIMER_REMOVE_FIRST, &TIMER_BLANK, &TIMER_REMOVE_FIRST],
            &mut ringer,
        )?;
        assert_eq!(ringer.timers(), countdowns(&[(1, 0)])?);
        Ok(())
    }

    #[test]
    fn a_tap_on_a_saved_timer_outside_its_play_and_cross_does_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30), (5, 0)])?;
        let before = ringer.clone();
        drain(
            &mut FakeInput::with(&[&TIMER_TAB, &TIMER_FIRST]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer, before);
        Ok(())
    }

    fn shown(
        clock: &FakeClock,
        ringer: &mut Ringer,
        light: &mut Light,
    ) -> Result<Frame, Box<dyn std::error::Error>> {
        let mut display = FakeDisplay::default();
        show(
            clock,
            UtcOffset::UTC,
            ringer,
            light,
            Gauge::UNREAD,
            &mut display,
        )?;
        Ok(display.shown.pop().ok_or("nothing shown")?)
    }

    #[test]
    fn a_saved_timer_played_from_the_list_rings_at_its_end_and_power_returns_to_the_list()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30), (5, 0)])?;
        let mut light = always();
        let list = |ringer: &Ringer| -> Result<Frame, Box<dyn std::error::Error>> {
            let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
            Ok(render(PANEL, &face))
        };
        drain_lit(
            &mut FakeInput::with(&[&TIMER_TAB]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let listed = list(&ringer)?;
        assert_eq!(ringer.stage(), Stage::List);
        drain_lit(
            &mut FakeInput::with(&[&PLAY_SECOND]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let ends = UnixSeconds(clock.now()?.0 + 300);
        assert_eq!(ringer.stage(), Stage::Running);
        assert_eq!(ringer.running_until(), Some(ends));
        assert_eq!(ringer.held(), Ring::Held(ends));
        clock.pass(1);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
        assert_eq!(
            face.running,
            Some(Running {
                left: Countdown::new(4, 59)?,
                length: Countdown::new(5, 0)?,
                paused: false,
            })
        );
        assert_ne!(shown(&clock, &mut ringer, &mut light)?, listed);
        clock.pass(298);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        clock.pass(1);
        shown(&clock, &mut ringer, &mut light)?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        assert_eq!(ringer.running_until(), None);
        assert_eq!(light.screen(), Screen::Timer);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
        assert!(face.ringing);
        assert_eq!(face.rung, Some(Countdown::new(5, 0)?));
        drain_lit(
            &mut FakeInput::with(&[&VOLUME, &PLAY_FIRST]),
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
        assert_eq!(ringer.stage(), Stage::List);
        assert_eq!(light.screen(), Screen::Timer);
        clock.pass(VOLUME_SECONDS);
        assert_eq!(shown(&clock, &mut ringer, &mut light)?, list(&ringer)?);
        Ok(())
    }

    #[test]
    fn pause_holds_the_time_left_and_resume_continues_it() -> Result<(), Box<dyn std::error::Error>>
    {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30)])?;
        let mut light = always();
        let mut on = |gestures: &[&[InputEvent]], ringer: &mut Ringer| {
            drain_lit(&mut FakeInput::with(gestures), &clock, ringer, &mut light)
        };
        on(&[&TIMER_TAB, &PLAY_FIRST], &mut ringer)?;
        clock.pass(30);
        on(&[&PAUSE], &mut ringer)?;
        assert_eq!(ringer.paused(), Some(Countdown::new(1, 0)?));
        assert_eq!(ringer.running_until(), None);
        assert_eq!(ringer.held(), Ring::Paused);
        assert_eq!(ringer.stage(), Stage::Running);
        clock.pass(600);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        let face = ringer.face(clock.now()?, UtcOffset::UTC, Gauge::UNREAD, Screen::Timer);
        assert_eq!(
            face.running,
            Some(Running {
                left: Countdown::new(1, 0)?,
                length: Countdown::new(1, 30)?,
                paused: true,
            })
        );
        on(&[&PAUSE], &mut ringer)?;
        let ends = UnixSeconds(clock.now()?.0 + 60);
        assert_eq!(ringer.paused(), None);
        assert_eq!(ringer.running_until(), Some(ends));
        assert_eq!(ringer.held(), Ring::Held(ends));
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Ringing);
        Ok(())
    }

    #[test]
    fn the_x_ends_a_paused_timer_and_returns_to_the_list_but_not_a_running_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30), (5, 0)])?;
        let mut light = always();
        let mut on = |gestures: &[&[InputEvent]], ringer: &mut Ringer| {
            drain_lit(&mut FakeInput::with(gestures), &clock, ringer, &mut light)
        };
        on(&[&TIMER_TAB, &PLAY_FIRST], &mut ringer)?;
        let ends = ringer.running_until();
        on(&[&CLOSE, &PLAY_SECOND], &mut ringer)?;
        assert_eq!(ringer.running_until(), ends);
        assert_eq!(ringer.stage(), Stage::Running);
        on(&[&PAUSE, &CLOSE], &mut ringer)?;
        assert_eq!(ringer.stage(), Stage::List);
        assert_eq!(ringer.running_until(), None);
        assert_eq!(ringer.paused(), None);
        assert_eq!(ringer.held(), Ring::Quiet);
        assert_eq!(ringer.timers(), countdowns(&[(1, 30), (5, 0)])?);
        clock.pass(90);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Quiet);
        on(&[&PLAY_SECOND], &mut ringer)?;
        assert_eq!(
            ringer.running_until(),
            Some(UnixSeconds(clock.now()?.0 + 300))
        );
        Ok(())
    }

    #[test]
    fn a_switch_to_another_tab_and_back_keeps_the_stage_and_the_timer()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 0, 0);
        let mut ringer = timed(&[(1, 30)])?;
        let mut light = always();
        let mut on = |gestures: &[&[InputEvent]], ringer: &mut Ringer| {
            drain_lit(&mut FakeInput::with(gestures), &clock, ringer, &mut light)
        };
        on(&[&TIMER_TAB, &PLAY_FIRST], &mut ringer)?;
        let ends = ringer.running_until();
        on(
            &[&ALARM_TAB, &FIRST, &SETTINGS_TAB, &TIMER_TAB],
            &mut ringer,
        )?;
        assert_eq!(ringer.stage(), Stage::Running);
        assert_eq!(ringer.running_until(), ends);
        assert!(ringer.armed());
        clock.pass(10);
        on(&[&PAUSE, &ALARM_TAB, &TIMER_TAB], &mut ringer)?;
        assert_eq!(ringer.stage(), Stage::Running);
        assert_eq!(ringer.paused(), Some(Countdown::new(1, 20)?));
        on(&[&CLOSE, &SETTINGS_TAB, &TIMER_TAB], &mut ringer)?;
        assert_eq!(ringer.stage(), Stage::List);
        Ok(())
    }

    #[test]
    fn play_on_an_empty_row_starts_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(12, 0, 0);
        let mut ringer = timed(&[(1, 30)])?;
        drain(
            &mut FakeInput::with(&[&TIMER_TAB, &PLAY_SECOND]),
            &clock,
            &mut ringer,
        )?;
        assert_eq!(ringer.stage(), Stage::List);
        assert_eq!(timed(&[])?.held(), Ring::Quiet);
        Ok(())
    }

    #[test]
    fn a_running_timer_survives_power_and_holds_the_phone_after_a_dismissed_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = timed(&[(5, 0)])?;
        drain(
            &mut FakeInput::with(&[&FIRST, &TIMER_TAB, &PLAY_FIRST, &DISMISS]),
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
            &mut FakeInput::with(&[&SETTINGS_TAB, &FIRST, &SECOND, &REMOVE_FIRST, &BLANK]),
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
    fn a_first_tap_on_the_recovery_row_marks_it_and_a_second_asks_for_recovery_once()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let armed = ringer.clone();
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &RECOVERY]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let face = ringer.face(
            clock.now()?,
            UtcOffset::UTC,
            Gauge::UNREAD,
            Screen::Settings,
        );
        assert!(face.recovery_marked);
        assert!(!ringer.take_recovery());
        drain_lit(
            &mut FakeInput::with(&[&RECOVERY]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let face = ringer.face(
            clock.now()?,
            UtcOffset::UTC,
            Gauge::UNREAD,
            Screen::Settings,
        );
        assert!(!face.recovery_marked);
        assert!(ringer.take_recovery());
        assert!(!ringer.take_recovery());
        assert_eq!(ringer, armed);
        Ok(())
    }

    #[test]
    fn any_other_touch_between_the_two_taps_asks_for_no_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        let mut light = always();
        for other in [&BLANK, &LIT_TIME, &SETTINGS_TAB] {
            drain_lit(
                &mut FakeInput::with(&[&SETTINGS_TAB, &RECOVERY, other, &RECOVERY, &BLANK]),
                &clock,
                &mut ringer,
                &mut light,
            )?;
            assert!(!ringer.take_recovery());
        }
        let face = ringer.face(
            clock.now()?,
            UtcOffset::UTC,
            Gauge::UNREAD,
            Screen::Settings,
        );
        assert!(!face.recovery_marked);
        Ok(())
    }

    #[test]
    fn the_panel_going_off_or_the_alarm_ringing_clears_the_recovery_mark()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = FakeClock::at(6, 59, 0);
        let mut ringer = ringer(7, 0)?;
        drain(&mut FakeInput::with(&[&FIRST]), &clock, &mut ringer)?;
        let mut light = Light::lit_at(clock.now()?, LIT_SECONDS);
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &RECOVERY]),
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
        let face = ringer.face(
            clock.now()?,
            UtcOffset::UTC,
            Gauge::UNREAD,
            Screen::Settings,
        );
        assert!(!face.recovery_marked);
        let mut light = always();
        drain_lit(
            &mut FakeInput::with(&[&SETTINGS_TAB, &RECOVERY]),
            &clock,
            &mut ringer,
            &mut light,
        )?;
        let face = ringer.face(
            clock.now()?,
            UtcOffset::UTC,
            Gauge::UNREAD,
            Screen::Settings,
        );
        assert!(face.recovery_marked);
        clock.pass(60);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        assert_eq!(ringer.ring(), Ring::Ringing);
        let face = ringer.face(
            clock.now()?,
            UtcOffset::UTC,
            Gauge::UNREAD,
            Screen::Settings,
        );
        assert!(!face.recovery_marked);
        assert!(!ringer.take_recovery());
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
        on(&[&TIMER_TAB, &TIMER_MINUTES_ABOVE], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::new(1, 0)?);
        on(&[&TIMER_SECONDS_ABOVE], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::new(1, 1)?);
        on(&[&TIMER_MINUTES_BELOW, &TIMER_MINUTES_BELOW], &mut timer)?;
        assert_eq!(timer.countdown(), Countdown::new(99, 1)?);
        on(&[&TIMER_SECONDS_BELOW, &TIMER_SECONDS_BELOW], &mut timer)?;
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
