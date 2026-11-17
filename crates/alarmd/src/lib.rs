use alarm_core::battery::{ChargeLimit, ChargeStep, stop_at_limit, wants_mark};
use alarm_core::hardware::{Battery, Clock, Display, HardwareError, Suspend, WakeAlarm};
use alarm_core::light::Light;
use alarm_core::ringer::{Ringer, show};
use alarm_core::time::{UnixSeconds, UtcOffset, seconds_to_next_minute};
use alarm_core::window::{Answer, Window};
use pid1::kmsg;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const LOCAL_OFFSET_SECONDS: i32 = 3_600;
pub const BRIGHTNESS: u32 = 2_000;
pub const RETRY_SECONDS: u64 = 10;
pub const WOKEN_SECONDS: u64 = 1;
pub const ALARM_HOUR: u8 = 7;
pub const ALARM_MINUTE: u8 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Minute {
    pub log: Vec<String>,
    pub wait_seconds: u64,
}

pub fn minute(
    clock: &impl Clock,
    offset: UtcOffset,
    battery: &mut Result<impl Battery, HardwareError>,
    ringer: &mut Ringer,
    light: &mut Light,
    display: &mut impl Display,
) -> Minute {
    let mut log = Vec::new();
    let charge = match battery {
        Ok(battery) => stop_at_limit(ChargeLimit::DEFAULT, battery),
        Err(error) => Err(error.clone()),
    };
    match charge {
        Ok(ChargeStep::Stopped) => log.push(kmsg::line("charge stopped at the limit")),
        Ok(ChargeStep::Left) => {}
        Err(error) => log.push(kmsg::failure("holding the charge limit", &error)),
    }
    let low = battery.as_ref().map_or(true, wants_mark);
    let wait_seconds = match show(clock, offset, ringer, light, low, display) {
        Ok(now) => light
            .seconds_left(now)
            .map_or(seconds_to_next_minute(now), |left| {
                left.min(seconds_to_next_minute(now))
            }),
        Err(error) => {
            log.push(kmsg::failure("showing the time", &error));
            RETRY_SECONDS
        }
    };
    Minute { log, wait_seconds }
}

enum Rest {
    Awake,
    Armed(UnixSeconds),
    Disarmed,
}

fn suspended(
    clock: &impl Clock,
    ringer: &Ringer,
    window: &mut Window,
    wake: &mut Result<impl WakeAlarm, HardwareError>,
    suspend: &mut Result<impl Suspend, HardwareError>,
) -> Result<Rest, HardwareError> {
    let now = clock.now()?;
    window.dark(now);
    let until = match window.answer(now, ringer.ring(), ringer.next()) {
        Answer::Suspend(at) if at > now => Some(at),
        Answer::PowerOff => ringer.next(),
        Answer::Lit | Answer::Suspend(_) => return Ok(Rest::Awake),
    };
    let wake = wake.as_mut().map_err(|error| error.clone())?;
    let suspend = suspend.as_mut().map_err(|error| error.clone())?;
    let rest = match until {
        Some(at) => wake.arm(at).map(|()| Rest::Armed(at))?,
        None => wake.disarm().map(|()| Rest::Disarmed)?,
    };
    suspend.suspend()?;
    Ok(rest)
}

pub fn rest(
    clock: &impl Clock,
    ringer: &Ringer,
    light: &Light,
    window: &mut Window,
    wake: &mut Result<impl WakeAlarm, HardwareError>,
    suspend: &mut Result<impl Suspend, HardwareError>,
    wait_seconds: u64,
) -> Minute {
    if light.lit() {
        window.light();
        return Minute {
            log: Vec::new(),
            wait_seconds,
        };
    }
    let resumed = |text: &str| Minute {
        log: vec![kmsg::line(text)],
        wait_seconds: WOKEN_SECONDS,
    };
    match suspended(clock, ringer, window, wake, suspend) {
        Ok(Rest::Awake) => Minute {
            log: Vec::new(),
            wait_seconds,
        },
        Ok(Rest::Armed(at)) => resumed(&format!("resumed from a suspend armed for {}", at.0)),
        Ok(Rest::Disarmed) => resumed("resumed from a suspend with the RTC disarmed"),
        Err(error) => Minute {
            log: vec![kmsg::failure("suspending", &error)],
            wait_seconds: wait_seconds.min(RETRY_SECONDS),
        },
    }
}

#[must_use]
pub fn change(before: &Ringer, after: &Ringer) -> Option<String> {
    let alarm = after.alarm();
    let armed = if after.armed() { "armed" } else { "disarmed" };
    (before != after).then(|| {
        kmsg::line(&format!(
            "alarm {:02}:{:02} {armed}, {:?}",
            alarm.hour(),
            alarm.minute(),
            after.ring()
        ))
    })
}

#[cfg(test)]
mod tests {
    use alarm_core::battery::{BatteryReading, ChargeStatus, Health, Percent};
    use alarm_core::face::{Face, Frame, RINGING, Size, WARNING, render};
    use alarm_core::input::InputEvent;
    use alarm_core::light::LIT_SECONDS;
    use alarm_core::time::{TimeError, TimeOfDay};
    use alarm_core::window::WINDOW_SECONDS;

    use super::*;

    const SIZE: Size = Size {
        width: 100,
        height: 112,
    };

    struct FixedClock(Result<UnixSeconds, HardwareError>);

    impl Clock for FixedClock {
        fn now(&self) -> Result<UnixSeconds, HardwareError> {
            self.0.clone()
        }
    }

    struct FakeBattery {
        capacity: Percent,
        status: ChargeStatus,
        enabled: bool,
    }

    impl Battery for FakeBattery {
        fn read(&self) -> Result<BatteryReading, HardwareError> {
            Ok(BatteryReading {
                capacity: self.capacity,
                status: self.status,
                microvolts: 4_000_000,
                decicelsius: 250,
                milliamps: 100,
                health: Health::Good,
            })
        }

        fn charger_enabled(&self) -> Result<bool, HardwareError> {
            Ok(self.enabled)
        }

        fn set_charger_enabled(&mut self, enabled: bool) -> Result<(), HardwareError> {
            self.enabled = enabled;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeDisplay {
        shown: Vec<Frame>,
        lit: Vec<bool>,
    }

    impl Display for FakeDisplay {
        fn size(&self) -> Size {
            SIZE
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

    #[derive(Default)]
    struct FakeRtc {
        armed: Vec<Option<UnixSeconds>>,
        failing: bool,
    }

    impl WakeAlarm for FakeRtc {
        fn arm(&mut self, at: UnixSeconds) -> Result<(), HardwareError> {
            if self.failing {
                return Err(refused("rtc0/wakealarm"));
            }
            self.armed.push(Some(at));
            Ok(())
        }

        fn disarm(&mut self) -> Result<(), HardwareError> {
            if self.failing {
                return Err(refused("rtc0/wakealarm"));
            }
            self.armed.push(None);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeSuspend {
        suspends: u32,
        failing: bool,
    }

    impl Suspend for FakeSuspend {
        fn suspend(&mut self) -> Result<(), HardwareError> {
            if self.failing {
                return Err(refused("power/state"));
            }
            self.suspends += 1;
            Ok(())
        }
    }

    fn refused(device: &'static str) -> HardwareError {
        HardwareError::Failed {
            device,
            reason: "refused".to_owned(),
        }
    }

    struct Resting {
        window: Window,
        wake: Result<FakeRtc, HardwareError>,
        suspend: Result<FakeSuspend, HardwareError>,
    }

    impl Resting {
        fn new() -> Self {
            Self {
                window: Window::lit(WINDOW_SECONDS),
                wake: Ok(FakeRtc::default()),
                suspend: Ok(FakeSuspend::default()),
            }
        }

        fn rest(&mut self, clock: &FixedClock, ringer: &Ringer, light: &Light) -> Minute {
            rest(
                clock,
                ringer,
                light,
                &mut self.window,
                &mut self.wake,
                &mut self.suspend,
                30,
            )
        }

        fn armed(&self) -> Option<&[Option<UnixSeconds>]> {
            self.wake.as_ref().ok().map(|rtc| rtc.armed.as_slice())
        }

        fn suspends(&self) -> Option<u32> {
            self.suspend.as_ref().ok().map(|suspend| suspend.suspends)
        }
    }

    fn battery(capacity: u8, status: ChargeStatus) -> Result<FakeBattery, HardwareError> {
        let capacity = Percent::new(capacity).map_err(|error| HardwareError::Failed {
            device: "fake",
            reason: error.to_string(),
        })?;
        Ok(FakeBattery {
            capacity,
            status,
            enabled: true,
        })
    }

    fn lit() -> Light {
        Light::lit_at(UnixSeconds(0), i64::MAX / 2)
    }

    fn ringer() -> Result<Ringer, TimeError> {
        Ok(Ringer::new(TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?))
    }

    fn armed_at(clock: &FixedClock) -> Result<Ringer, Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        ringer.heard(
            InputEvent::Touch { x: 0, y: 0 },
            SIZE,
            clock.now()?,
            UtcOffset::UTC,
        );
        Ok(ringer)
    }

    fn dark() -> Light {
        Light::dark(LIT_SECONDS)
    }

    fn after(clock: &FixedClock, seconds: i64) -> Result<UnixSeconds, HardwareError> {
        Ok(UnixSeconds(clock.now()?.0 + seconds))
    }

    fn face(hour: u8, minute: u8, low_battery: bool) -> Result<Face, TimeError> {
        Ok(Face {
            time: TimeOfDay::new(hour, minute)?,
            alarm: TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?,
            armed: false,
            ringing: false,
            low_battery,
        })
    }

    fn at(hour: i64, minute: i64, second: i64) -> FixedClock {
        FixedClock(Ok(UnixSeconds(
            20_768 * 86_400 + hour * 3_600 + minute * 60 + second,
        )))
    }

    #[test]
    fn a_minute_draws_the_local_time_and_sleeps_to_the_next()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let offset = UtcOffset::from_seconds(LOCAL_OFFSET_SECONDS)?;
        let step = minute(
            &at(19, 37, 58),
            offset,
            &mut battery,
            &mut ringer()?,
            &mut lit(),
            &mut display,
        );
        assert_eq!(
            step,
            Minute {
                log: Vec::new(),
                wait_seconds: 2,
            }
        );
        assert_eq!(display.shown, [render(SIZE, &face(20, 37, false)?)]);
        Ok(())
    }

    #[test]
    fn at_the_limit_the_charge_stops_and_says_so() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(80, ChargeStatus::Charging);
        let mut display = FakeDisplay::default();
        let step = minute(
            &at(6, 0, 0),
            UtcOffset::UTC,
            &mut battery,
            &mut ringer()?,
            &mut lit(),
            &mut display,
        );
        assert_eq!(step.log, [kmsg::line("charge stopped at the limit")]);
        assert!(!battery?.enabled);
        Ok(())
    }

    #[test]
    fn a_low_battery_puts_the_mark_on_the_face() -> Result<(), TimeError> {
        let mut battery = battery(20, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        minute(
            &at(6, 0, 0),
            UtcOffset::UTC,
            &mut battery,
            &mut ringer()?,
            &mut lit(),
            &mut display,
        );
        assert!(
            display
                .shown
                .iter()
                .all(|frame| (0..SIZE.height).any(|y| frame.row(y).contains(&WARNING)))
        );
        assert_eq!(display.shown.len(), 1);
        Ok(())
    }

    #[test]
    fn an_absent_battery_still_shows_the_time_with_the_mark() -> Result<(), TimeError> {
        let mut battery: Result<FakeBattery, HardwareError> =
            Err(HardwareError::Absent("Battery/capacity"));
        let mut display = FakeDisplay::default();
        let step = minute(
            &at(6, 0, 30),
            UtcOffset::UTC,
            &mut battery,
            &mut ringer()?,
            &mut lit(),
            &mut display,
        );
        assert_eq!(
            step.log,
            [kmsg::failure(
                "holding the charge limit",
                &HardwareError::Absent("Battery/capacity")
            )]
        );
        assert_eq!(step.wait_seconds, 30);
        assert_eq!(display.shown, [render(SIZE, &face(6, 0, true)?)]);
        Ok(())
    }

    #[test]
    fn a_failing_clock_draws_nothing_and_retries_soon() -> Result<(), TimeError> {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let clock = FixedClock(Err(HardwareError::Absent("realtime clock")));
        let step = minute(
            &clock,
            UtcOffset::UTC,
            &mut battery,
            &mut ringer()?,
            &mut lit(),
            &mut display,
        );
        assert_eq!(
            step,
            Minute {
                log: vec![kmsg::failure(
                    "showing the time",
                    &HardwareError::Absent("realtime clock")
                )],
                wait_seconds: RETRY_SECONDS,
            }
        );
        assert_eq!(display.shown, Vec::<Frame>::new());
        Ok(())
    }

    #[test]
    fn an_armed_alarm_turns_the_face_amber_at_its_minute() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let mut ringer = ringer()?;
        let before = at(6, 59, 0);
        ringer.heard(
            InputEvent::Touch { x: 0, y: 0 },
            SIZE,
            before.now()?,
            UtcOffset::UTC,
        );
        minute(
            &before,
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut lit(),
            &mut display,
        );
        minute(
            &at(7, 0, 0),
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut lit(),
            &mut display,
        );
        let amber = |frame: &Frame| frame.row(0).contains(&RINGING);
        assert_eq!(
            display.shown.iter().map(amber).collect::<Vec<_>>(),
            [false, true]
        );
        Ok(())
    }

    #[test]
    fn a_change_to_the_ringer_is_logged_once() -> Result<(), Box<dyn std::error::Error>> {
        let before = ringer()?;
        let mut after = before;
        assert_eq!(change(&before, &after), None);
        after.heard(
            InputEvent::Touch { x: 0, y: 0 },
            SIZE,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&before, &after),
            Some(kmsg::line("alarm 07:00 armed, Quiet"))
        );
        Ok(())
    }

    #[test]
    fn a_lit_panel_wakes_the_loop_when_it_is_due_off() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let clock = at(19, 37, 30);
        let mut light = Light::lit_at(clock.now()?, LIT_SECONDS);
        let step = minute(
            &clock,
            UtcOffset::UTC,
            &mut battery,
            &mut ringer()?,
            &mut light,
            &mut display,
        );
        assert_eq!(step.wait_seconds, 10);
        assert_eq!(display.lit, [true]);
        assert_eq!(display.shown.len(), 1);
        Ok(())
    }

    #[test]
    fn on_the_cable_the_panel_goes_off_all_the_same() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(55, ChargeStatus::Charging);
        let mut display = FakeDisplay::default();
        let mut light = Light::lit_at(at(19, 37, 30).now()?, LIT_SECONDS);
        let mut ringer = ringer()?;
        let step = minute(
            &at(19, 37, 40),
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut light,
            &mut display,
        );
        assert_eq!(step.wait_seconds, 20);
        assert_eq!(display.lit, [false]);
        assert_eq!(display.shown, Vec::<Frame>::new());
        assert!(battery?.enabled);
        Ok(())
    }

    #[test]
    fn the_panel_going_off_suspends_until_the_window_ends() -> Result<(), Box<dyn std::error::Error>>
    {
        let clock = at(19, 37, 30);
        let mut resting = Resting::new();
        let step = resting.rest(&clock, &ringer()?, &dark());
        let ends = after(&clock, WINDOW_SECONDS)?;
        assert_eq!(
            step,
            Minute {
                log: vec![kmsg::line(&format!(
                    "resumed from a suspend armed for {}",
                    ends.0
                ))],
                wait_seconds: WOKEN_SECONDS,
            }
        );
        assert_eq!(resting.armed(), Some([Some(ends)].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn a_lit_panel_stays_awake_and_ends_the_window() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut resting = Resting::new();
        resting.window.dark(clock.now()?);
        let step = resting.rest(&clock, &ringer()?, &lit());
        assert_eq!(
            step,
            Minute {
                log: Vec::new(),
                wait_seconds: 30,
            }
        );
        assert!(!resting.window.running());
        assert_eq!(resting.armed(), Some([].as_slice()));
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }

    #[test]
    fn an_alarm_inside_the_window_is_what_the_rtc_wakes_for()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(6, 59, 0);
        let mut resting = Resting::new();
        resting.rest(&clock, &armed_at(&clock)?, &dark());
        assert_eq!(resting.armed(), Some([Some(at(7, 0, 0).now()?)].as_slice()));
        Ok(())
    }

    #[test]
    fn after_the_window_the_rtc_is_armed_for_the_next_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = armed_at(&clock)?;
        let mut resting = Resting::new();
        resting.window.dark(at(19, 35, 0).now()?);
        resting.rest(&clock, &ringer, &dark());
        assert_eq!(ringer.next(), Some(at(31, 0, 0).now()?));
        assert_eq!(resting.armed(), Some([ringer.next()].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn after_the_window_with_no_alarm_the_rtc_is_disarmed() -> Result<(), Box<dyn std::error::Error>>
    {
        let clock = at(19, 37, 30);
        let mut resting = Resting::new();
        resting.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer()?, &dark());
        assert_eq!(
            step.log,
            [kmsg::line("resumed from a suspend with the RTC disarmed")]
        );
        assert_eq!(resting.armed(), Some([None].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn a_press_after_the_wake_ends_the_window_and_the_next_dark_starts_another()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut resting = Resting::new();
        let ringer = ringer()?;
        resting.rest(&at(19, 37, 0), &ringer, &dark());
        resting.rest(&at(19, 37, 5), &ringer, &lit());
        resting.rest(&at(19, 38, 10), &ringer, &dark());
        assert_eq!(
            resting.armed(),
            Some([Some(at(19, 39, 0).now()?), Some(at(19, 40, 10).now()?)].as_slice())
        );
        assert_eq!(resting.suspends(), Some(2));
        Ok(())
    }

    #[test]
    fn a_failed_arm_does_not_suspend_and_retries_soon() -> Result<(), Box<dyn std::error::Error>> {
        let mut resting = Resting::new();
        resting.wake = Ok(FakeRtc {
            armed: Vec::new(),
            failing: true,
        });
        let step = resting.rest(&at(19, 37, 30), &ringer()?, &dark());
        assert_eq!(
            step,
            Minute {
                log: vec![kmsg::failure("suspending", &refused("rtc0/wakealarm"))],
                wait_seconds: RETRY_SECONDS,
            }
        );
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }

    #[test]
    fn a_suspend_the_kernel_aborts_is_logged_and_retried() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut resting = Resting::new();
        resting.suspend = Ok(FakeSuspend {
            suspends: 0,
            failing: true,
        });
        let step = resting.rest(&at(19, 37, 30), &ringer()?, &dark());
        assert_eq!(
            step,
            Minute {
                log: vec![kmsg::failure("suspending", &refused("power/state"))],
                wait_seconds: RETRY_SECONDS,
            }
        );
        assert_eq!(resting.armed().map(<[_]>::len), Some(1));
        Ok(())
    }

    #[test]
    fn an_absent_rtc_never_suspends() -> Result<(), Box<dyn std::error::Error>> {
        let mut resting = Resting::new();
        resting.wake = Err(HardwareError::Absent("rtc0/wakealarm"));
        let step = resting.rest(&at(19, 37, 30), &ringer()?, &dark());
        assert_eq!(
            step.log,
            [kmsg::failure(
                "suspending",
                &HardwareError::Absent("rtc0/wakealarm")
            )]
        );
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }

    #[test]
    fn a_failing_clock_does_not_suspend() -> Result<(), Box<dyn std::error::Error>> {
        let mut resting = Resting::new();
        let clock = FixedClock(Err(HardwareError::Absent("realtime clock")));
        let step = resting.rest(&clock, &ringer()?, &dark());
        assert_eq!(step.wait_seconds, RETRY_SECONDS);
        assert!(!resting.window.running());
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }
}
