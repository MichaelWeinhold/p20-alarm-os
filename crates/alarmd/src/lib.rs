use alarm_core::alarm::{Alarm, Weekdays};
use alarm_core::battery::{ChargeLimit, ChargeStep, Gauge, stop_at_limit};
use std::thread::JoinHandle;

use alarm_core::hardware::{
    Battery, BootReason, Clock, Display, HardwareError, Indicator, Modem, PowerOff, Settings,
    Speaker, Suspend, Vibrator, WakeAlarm,
};
use alarm_core::light::Light;
use alarm_core::ringer::{Ringer, show};
use alarm_core::settings::{self, Choices, SetAlarm};
use alarm_core::sound::Sound;
use alarm_core::time::{Countdown, TimeOfDay, UnixSeconds, UtcOffset, seconds_to_next_minute};
use alarm_core::vibration::Vibration;
use alarm_core::warning::Warning;
use alarm_core::window::{Answer, WINDOW_SECONDS, Window, power_on_at};
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
    let gauge = battery.as_ref().map_or(Gauge::UNREAD, |battery| {
        Gauge::read(battery, ChargeLimit::DEFAULT)
    });
    let wait_seconds = match show(clock, offset, ringer, light, gauge, display) {
        Ok(now) => {
            let timer = ringer.running_until().map(|ends| {
                if light.lit() {
                    WOKEN_SECONDS
                } else {
                    ends.0.saturating_sub(now.0).max(1).unsigned_abs()
                }
            });
            [light.seconds_left(now), timer]
                .into_iter()
                .flatten()
                .fold(seconds_to_next_minute(now), u64::min)
        }
        Err(error) => {
            log.push(kmsg::failure("showing the time", &error));
            RETRY_SECONDS
        }
    };
    Minute { log, wait_seconds }
}

#[derive(Debug)]
pub struct Buzz<V, S> {
    pub vibration: Vibration,
    pub vibrator: Result<V, HardwareError>,
    pub sound: Sound,
    pub speaker: Result<S, HardwareError>,
}

pub fn pulse(
    clock: &impl Clock,
    ringer: &Ringer,
    buzz: &mut Buzz<impl Vibrator, impl Speaker>,
    wait_seconds: u64,
) -> Minute {
    let mut log = Vec::new();
    let ring = ringer.ring();
    if let Ok(speaker) = &mut buzz.speaker
        && let Err(error) = buzz.sound.tick(ring, speaker)
    {
        log.push(kmsg::failure("driving the speaker", &error));
    }
    let (Ok(vibrator), Ok(now)) = (&mut buzz.vibrator, clock.now()) else {
        return Minute { log, wait_seconds };
    };
    if let Err(error) = buzz.vibration.tick(now, ring, vibrator) {
        log.push(kmsg::failure("driving the vibrator", &error));
    }
    let wait_seconds = buzz
        .vibration
        .seconds_left(now)
        .map_or(wait_seconds, |left| left.min(wait_seconds));
    Minute { log, wait_seconds }
}

#[derive(Debug)]
pub struct Lamp<I> {
    pub warning: Warning,
    pub indicator: Result<I, HardwareError>,
}

pub fn warned(
    battery: &Result<impl Battery, HardwareError>,
    lamp: &mut Lamp<impl Indicator>,
) -> Vec<String> {
    let Ok(indicator) = &mut lamp.indicator else {
        return Vec::new();
    };
    let reading = battery
        .as_ref()
        .ok()
        .and_then(|battery| battery.read().ok());
    lamp.warning
        .tick(reading, indicator)
        .err()
        .map(|error| kmsg::failure("driving the low-battery warning", &error))
        .into_iter()
        .collect()
}

#[derive(Debug)]
pub struct Sleep<W, S, P> {
    pub window: Window,
    pub wake: Result<W, HardwareError>,
    pub suspend: Result<S, HardwareError>,
    pub power: Result<P, HardwareError>,
}

enum Rest {
    Awake,
    Armed(UnixSeconds),
    Disarmed,
    PoweredOff(Option<UnixSeconds>),
}

pub fn cable_in(battery: &Result<impl Battery, HardwareError>) -> Result<bool, HardwareError> {
    let battery = battery.as_ref().map_err(Clone::clone)?;
    Ok(battery.read()?.status.cable_in())
}

fn powered_off(
    on_at: Option<UnixSeconds>,
    cable_in: Result<bool, HardwareError>,
    power: &mut Result<impl PowerOff, HardwareError>,
    log: &mut Vec<String>,
) -> bool {
    let outcome = match cable_in {
        Ok(true) => return false,
        Ok(false) => power
            .as_mut()
            .map_err(|error| error.clone())
            .and_then(|power| power.power_off(on_at))
            .map_err(|error| kmsg::failure("powering off", &error)),
        Err(error) => Err(kmsg::failure("reading the cable", &error)),
    };
    outcome.map_err(|line| log.push(line)).is_ok()
}

fn suspended(
    clock: &impl Clock,
    ringer: &Ringer,
    cable_in: Result<bool, HardwareError>,
    sleep: &mut Sleep<impl WakeAlarm, impl Suspend, impl PowerOff>,
    log: &mut Vec<String>,
) -> Result<Rest, HardwareError> {
    let now = clock.now()?;
    sleep.window.dark(now);
    let until = match sleep.window.answer(now, ringer.held(), ringer.next()) {
        Answer::Suspend(at) if at > now => Some(at),
        Answer::PowerOff => {
            let on_at = ringer.next().map(power_on_at);
            if ringer.power_off() && powered_off(on_at, cable_in, &mut sleep.power, log) {
                return Ok(Rest::PoweredOff(on_at));
            }
            ringer.next()
        }
        Answer::Lit | Answer::Suspend(_) => return Ok(Rest::Awake),
    };
    let wake = sleep.wake.as_mut().map_err(|error| error.clone())?;
    let suspend = sleep.suspend.as_mut().map_err(|error| error.clone())?;
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
    cable_in: Result<bool, HardwareError>,
    sleep: &mut Sleep<impl WakeAlarm, impl Suspend, impl PowerOff>,
    wait_seconds: u64,
) -> Minute {
    if light.lit() {
        sleep.window.light();
        return Minute {
            log: Vec::new(),
            wait_seconds,
        };
    }
    let mut log = Vec::new();
    let (line, wait_seconds) = match suspended(clock, ringer, cable_in, sleep, &mut log) {
        Ok(Rest::Awake) => (None, wait_seconds),
        Ok(Rest::Armed(at)) => (
            Some(kmsg::line(&format!(
                "resumed from a suspend armed for {}",
                at.0
            ))),
            WOKEN_SECONDS,
        ),
        Ok(Rest::Disarmed) => (
            Some(kmsg::line("resumed from a suspend with the RTC disarmed")),
            WOKEN_SECONDS,
        ),
        Ok(Rest::PoweredOff(on_at)) => (
            Some(kmsg::line(&format!(
                "powered off, on again at {:?}",
                on_at.map(|at| at.0)
            ))),
            WOKEN_SECONDS,
        ),
        Err(error) => (
            Some(kmsg::failure("suspending", &error)),
            wait_seconds.min(RETRY_SECONDS),
        ),
    };
    log.extend(line);
    Minute { log, wait_seconds }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Booted {
    pub light: Light,
    pub log: Vec<String>,
}

pub fn booted(
    boot: &Result<impl BootReason, HardwareError>,
    clock: &impl Clock,
    lit_seconds: i64,
) -> Booted {
    let mut log = Vec::new();
    let lit = match boot
        .as_ref()
        .map_err(Clone::clone)
        .and_then(BootReason::read)
    {
        Ok(boot) => {
            log.push(kmsg::line(&format!("booted by {boot:?}")));
            boot.lit()
        }
        Err(error) => {
            log.push(kmsg::failure("reading the boot reason", &error));
            true
        }
    };
    let light = match clock.now() {
        Ok(now) if lit => Light::lit_at(now, lit_seconds),
        Ok(_) => Light::dark(lit_seconds),
        Err(error) => {
            log.push(kmsg::failure("reading the clock", &error));
            Light::dark(lit_seconds)
        }
    };
    Booted { light, log }
}

#[must_use]
pub fn choices(ringer: &Ringer, offset: UtcOffset, window_seconds: i64) -> Choices {
    Choices {
        offset,
        window_seconds,
        power_off: ringer.power_off(),
        alarms: ringer
            .alarms()
            .iter()
            .enumerate()
            .map(|(index, &time)| SetAlarm {
                alarm: Alarm {
                    time,
                    repeat: Weekdays::EVERY_DAY,
                },
                armed: ringer.active_index() == Some(index) && ringer.armed(),
            })
            .collect(),
        timers: ringer.timers().to_vec(),
        lit_seconds: ringer.lit_seconds(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    pub saved: Choices,
    pub ringer: Ringer,
    pub log: Vec<String>,
}

pub fn restored(
    settings: &mut Result<impl Settings, HardwareError>,
    clock: &impl Clock,
    alarm: TimeOfDay,
    offset: UtcOffset,
) -> Restored {
    let mut log = Vec::new();
    let loaded = match settings
        .as_mut()
        .map_err(|error| error.clone())
        .and_then(settings::load)
    {
        Ok(Some(loaded)) => {
            log.push(kmsg::line("settings loaded"));
            loaded
        }
        Ok(None) => {
            log.push(kmsg::line("settings never saved"));
            choices(&Ringer::new(alarm), offset, WINDOW_SECONDS)
        }
        Err(error) => {
            log.push(kmsg::failure("loading the settings", &error));
            choices(&Ringer::new(alarm), offset, WINDOW_SECONDS)
        }
    };
    let armed = loaded
        .alarms
        .iter()
        .find(|set| set.armed)
        .map(|set| set.alarm.time);
    let picked = armed
        .or_else(|| loaded.alarms.first().map(|set| set.alarm.time))
        .unwrap_or(alarm);
    let mut ringer = Ringer::listed(
        loaded.alarms.iter().map(|set| set.alarm.time).collect(),
        picked,
    );
    ringer.set_power_off(loaded.power_off);
    ringer.set_lit_seconds(loaded.lit_seconds);
    ringer.set_timers(loaded.timers.clone());
    if let Some(armed) = armed {
        match clock.now() {
            Ok(now) => {
                if let Some(index) = ringer.alarms().iter().position(|&time| time == armed) {
                    ringer.activate(index, now, loaded.offset);
                }
            }
            Err(error) => log.push(kmsg::failure("arming the saved alarm", &error)),
        }
    }
    Restored {
        saved: choices(&ringer, loaded.offset, loaded.window_seconds),
        ringer,
        log,
    }
}

pub fn persisted(
    settings: &mut Result<impl Settings, HardwareError>,
    saved: &mut Choices,
    current: Choices,
) -> Option<String> {
    if current == *saved {
        return None;
    }
    let outcome = settings
        .as_mut()
        .map_err(|error| error.clone())
        .and_then(|settings| settings::save(settings, &current));
    Some(match outcome {
        Ok(()) => {
            *saved = current;
            kmsg::line("settings saved")
        }
        Err(error) => kmsg::failure("saving the settings", &error),
    })
}

pub fn modem_booted(modem: &mut Result<impl Modem, HardwareError>) -> Vec<String> {
    let modem = match modem {
        Ok(modem) => modem,
        Err(error) => return vec![kmsg::failure("opening the modem", error)],
    };
    if let Err(error) = modem.boot() {
        return vec![kmsg::failure("booting the modem", &error)];
    }
    let radio = match modem.radio_off() {
        Ok(()) => kmsg::line("modem radio off"),
        Err(error) => kmsg::failure("turning the modem's radio off", &error),
    };
    vec![kmsg::line("modem booted"), radio]
}

#[derive(Debug)]
pub struct AfterModem<S> {
    pub suspend: S,
    pub modem: Option<JoinHandle<()>>,
}

impl<S: Suspend> Suspend for AfterModem<S> {
    fn suspend(&mut self) -> Result<(), HardwareError> {
        if let Some(modem) = self.modem.take() {
            let _ = modem.join();
        }
        self.suspend.suspend()
    }
}

#[must_use]
pub fn change(before: &Ringer, after: &Ringer) -> Option<String> {
    let state = |ringer: &Ringer| {
        (
            ringer.alarms().to_vec(),
            ringer.next(),
            ringer.ring(),
            ringer.power_off(),
            ringer.lit_seconds(),
            ringer.timers().to_vec(),
            ringer.running_until(),
        )
    };
    let clock = |time: TimeOfDay| format!("{:02}:{:02}", time.hour(), time.minute());
    let length = |timer: Countdown| format!("{:02}:{:02}", timer.minutes(), timer.seconds());
    (state(before) != state(after)).then(|| {
        let alarms: Vec<String> = after.alarms().iter().map(|&time| clock(time)).collect();
        let timers: Vec<String> = after.timers().iter().map(|&timer| length(timer)).collect();
        let active = after.active().map_or_else(|| "none".to_owned(), clock);
        let power = if after.power_off() { "on" } else { "off" };
        let running = after
            .running_until()
            .map_or_else(|| "none".to_owned(), |ends| ends.0.to_string());
        kmsg::line(&format!(
            "alarms [{}], active {active}, {:?}, power-off {power}, lit {} s, timers [{}], running until {running}",
            alarms.join(", "),
            after.ring(),
            after.lit_seconds(),
            timers.join(", ")
        ))
    })
}

#[cfg(test)]
mod tests {
    use alarm_core::battery::{BatteryReading, ChargeStatus, Health, Percent};
    use alarm_core::boot::Boot;
    use alarm_core::face::{
        Face, Field, Frame, RINGING, Row, Screen, Size, Target, WARNING, render,
    };
    use alarm_core::input::Key;
    use alarm_core::light::LIT_SECONDS;
    use alarm_core::ringer::Ring;
    use alarm_core::settings::{RECORD_BYTES, SLOT_BYTES, Slot};
    use alarm_core::time::TimeError;
    use alarm_core::vibration::{PERIOD_SECONDS, PULSE_MILLISECONDS};

    use super::*;

    const SIZE: Size = Size {
        width: 108,
        height: 228,
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
    struct FakeVibrator {
        pulses: Vec<u32>,
        stops: u32,
        refusing: bool,
    }

    impl Vibrator for FakeVibrator {
        fn vibrate(&mut self, milliseconds: u32) -> Result<(), HardwareError> {
            self.pulses.push(milliseconds);
            if self.refusing {
                Err(refused("vibrator"))
            } else {
                Ok(())
            }
        }

        fn stop(&mut self) -> Result<(), HardwareError> {
            self.stops += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeSpeaker {
        starts: u32,
        stops: u32,
        refusing: bool,
    }

    impl Speaker for FakeSpeaker {
        fn start(&mut self) -> Result<(), HardwareError> {
            self.starts += 1;
            if self.refusing {
                Err(refused("speaker"))
            } else {
                Ok(())
            }
        }

        fn stop(&mut self) -> Result<(), HardwareError> {
            self.stops += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeIndicator {
        warnings: u32,
        clears: u32,
        refusing: bool,
    }

    impl Indicator for FakeIndicator {
        fn warn(&mut self) -> Result<(), HardwareError> {
            self.warnings += 1;
            if self.refusing {
                Err(refused("indicator"))
            } else {
                Ok(())
            }
        }

        fn clear(&mut self) -> Result<(), HardwareError> {
            self.clears += 1;
            Ok(())
        }
    }

    fn lamp(indicator: Result<FakeIndicator, HardwareError>) -> Lamp<FakeIndicator> {
        Lamp {
            warning: Warning::unset(),
            indicator,
        }
    }

    fn buzz(vibrator: Result<FakeVibrator, HardwareError>) -> Buzz<FakeVibrator, FakeSpeaker> {
        Buzz {
            vibration: Vibration::still(),
            vibrator,
            sound: Sound::quiet(),
            speaker: Ok(FakeSpeaker::default()),
        }
    }

    fn sounding(speaker: Result<FakeSpeaker, HardwareError>) -> Buzz<FakeVibrator, FakeSpeaker> {
        Buzz {
            speaker,
            ..buzz(Ok(FakeVibrator::default()))
        }
    }

    fn ringing_at(clock: &FixedClock) -> Result<Ringer, Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        let before = UnixSeconds(clock.now()?.0 - 60);
        ringer.touched(Target::Entry(0), Screen::Alarm, 0, before, UtcOffset::UTC);
        ringer.tick(clock.now()?, UtcOffset::UTC);
        Ok(ringer)
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

    #[derive(Debug, Default)]
    struct FakePower {
        offs: Vec<Option<UnixSeconds>>,
        failing: bool,
    }

    impl PowerOff for FakePower {
        fn power_off(&mut self, on_at: Option<UnixSeconds>) -> Result<(), HardwareError> {
            if self.failing {
                return Err(refused("reboot"));
            }
            self.offs.push(on_at);
            Ok(())
        }
    }

    struct FakeBoot(Result<Boot, HardwareError>);

    impl BootReason for FakeBoot {
        fn read(&self) -> Result<Boot, HardwareError> {
            self.0.clone()
        }
    }

    struct Resting {
        sleep: Sleep<FakeRtc, FakeSuspend, FakePower>,
        cable_in: Result<bool, HardwareError>,
    }

    impl Resting {
        fn new() -> Self {
            Self {
                sleep: Sleep {
                    window: Window::lit(WINDOW_SECONDS),
                    wake: Ok(FakeRtc::default()),
                    suspend: Ok(FakeSuspend::default()),
                    power: Ok(FakePower::default()),
                },
                cable_in: Ok(true),
            }
        }

        fn unplugged() -> Self {
            Self {
                cable_in: Ok(false),
                ..Self::new()
            }
        }

        fn rest(&mut self, clock: &FixedClock, ringer: &Ringer, light: &Light) -> Minute {
            rest(
                clock,
                ringer,
                light,
                self.cable_in.clone(),
                &mut self.sleep,
                30,
            )
        }

        fn armed(&self) -> Option<&[Option<UnixSeconds>]> {
            self.sleep
                .wake
                .as_ref()
                .ok()
                .map(|rtc| rtc.armed.as_slice())
        }

        fn suspends(&self) -> Option<u32> {
            self.sleep
                .suspend
                .as_ref()
                .ok()
                .map(|suspend| suspend.suspends)
        }

        fn offs(&self) -> Option<&[Option<UnixSeconds>]> {
            self.sleep
                .power
                .as_ref()
                .ok()
                .map(|power| power.offs.as_slice())
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

    fn running_at(clock: &FixedClock, seconds: u8) -> Result<Ringer, Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        ringer.set_timers(vec![Countdown::new(0, seconds)?]);
        ringer.touched(
            Target::Start,
            Screen::Timer,
            0,
            clock.now()?,
            UtcOffset::UTC,
        );
        Ok(ringer)
    }

    fn armed_at(clock: &FixedClock) -> Result<Ringer, Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        ringer.touched(
            Target::Entry(0),
            Screen::Alarm,
            0,
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

    fn face(hour: u8, minute: u8, battery: Gauge) -> Result<Face, TimeError> {
        Ok(Face {
            time: TimeOfDay::new(hour, minute)?,
            alarms: vec![TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?],
            active: None,
            marked: None,
            picked: TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?,
            ringing: false,
            battery,
            screen: Screen::Alarm,
            power_off: true,
            lit_seconds: LIT_SECONDS,
            timers: Vec::new(),
            countdown: Countdown::default(),
            left: None,
            rung: None,
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
        assert_eq!(
            display.shown,
            [render(
                SIZE,
                &face(20, 37, Gauge::of(Percent::new(55)?, ChargeLimit::DEFAULT))?
            )]
        );
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
    fn a_low_battery_draws_the_symbol_as_a_warning() -> Result<(), TimeError> {
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
    fn an_absent_battery_still_shows_the_time_with_the_low_symbol() -> Result<(), TimeError> {
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
        assert_eq!(display.shown, [render(SIZE, &face(6, 0, Gauge::UNREAD)?)]);
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
        ringer.touched(
            Target::Entry(0),
            Screen::Alarm,
            0,
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
        let mut after = before.clone();
        assert_eq!(change(&before, &after), None);
        after.touched(
            Target::Entry(0),
            Screen::Alarm,
            0,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&before, &after),
            Some(kmsg::line(
                "alarms [07:00], active 07:00, Quiet, power-off on, lit 10 s, timers [], running until none"
            ))
        );
        Ok(())
    }

    #[test]
    fn a_turned_toggle_is_logged() -> Result<(), Box<dyn std::error::Error>> {
        let before = ringer()?;
        let mut after = before.clone();
        after.touched(
            Target::PowerOff,
            Screen::Alarm,
            0,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&before, &after),
            Some(kmsg::line(
                "alarms [07:00], active none, Quiet, power-off off, lit 10 s, timers [], running until none"
            ))
        );
        Ok(())
    }

    #[test]
    fn a_started_timer_is_logged_with_its_end() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(6, 0, 0);
        let before = ringer()?;
        let started = running_at(&clock, 45)?;
        assert_eq!(
            change(&before, &started),
            Some(kmsg::line(&format!(
                "alarms [07:00], active none, Quiet, power-off on, lit 10 s, timers [00:45], running until {}",
                after(&clock, 45)?.0
            )))
        );
        Ok(())
    }

    #[test]
    fn a_running_timer_wakes_a_dark_loop_at_its_end_and_a_lit_one_each_second()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let clock = at(19, 37, 30);
        let mut ringer = running_at(&clock, 20)?;
        let dark = minute(
            &clock,
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut dark(),
            &mut display,
        );
        assert_eq!(dark.wait_seconds, 20);
        let lit = minute(
            &clock,
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut lit(),
            &mut display,
        );
        assert_eq!(lit.wait_seconds, WOKEN_SECONDS);
        Ok(())
    }

    #[test]
    fn a_running_timer_keeps_the_phone_on_past_the_window_and_the_rtc_armed_for_its_end()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = running_at(&clock, 50)?;
        let mut resting = Resting::unplugged();
        resting.sleep.window.dark(at(19, 30, 0).now()?);
        resting.rest(&clock, &ringer, &dark());
        assert_eq!(resting.offs(), Some([].as_slice()));
        assert_eq!(resting.armed(), Some([ringer.running_until()].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn a_moved_slider_is_not_logged() -> Result<(), Box<dyn std::error::Error>> {
        let before = ringer()?;
        let mut after = before.clone();
        after.touched(
            Target::Wheel(Field::Left, Row::Picked),
            Screen::Alarm,
            500,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        after.dragged(400, 46);
        assert_ne!(after.picked(), before.picked());
        assert_eq!(change(&before, &after), None);
        Ok(())
    }

    #[test]
    fn every_saved_alarm_is_restored_and_the_armed_one_rings_next()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let mut ringer = started.ringer.clone();
        for target in [
            Target::Wheel(Field::Left, Row::Above),
            Target::Add,
            Target::Wheel(Field::Left, Row::Below),
            Target::Wheel(Field::Left, Row::Below),
            Target::Add,
        ] {
            ringer.touched(target, Screen::Alarm, 0, clock.now()?, berlin()?);
            ringer.lifted();
        }
        ringer.touched(Target::Entry(2), Screen::Alarm, 0, clock.now()?, berlin()?);
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(current.alarms.iter().filter(|set| set.armed).count(), 1);
        persisted(&mut settings, &mut started.saved, current);

        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.alarms(), ringer.alarms());
        assert_eq!(rebooted.ringer.timers(), []);
        assert_eq!(rebooted.ringer.active(), ringer.active());
        assert_eq!(
            rebooted.ringer.next(),
            Some(Alarm::once(TimeOfDay::new(8, 0)?).next_after(at(20, 0, 0).now()?, berlin()?))
        );
        Ok(())
    }

    #[test]
    fn every_saved_timer_is_restored_with_the_shortest_on_the_wheels()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let mut ringer = started.ringer.clone();
        ringer.set_timers(vec![Countdown::new(12, 5)?, Countdown::new(3, 0)?]);
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(current.timers, ringer.timers());
        persisted(&mut settings, &mut started.saved, current);

        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.timers(), ringer.timers());
        assert_eq!(rebooted.ringer.countdown(), Countdown::new(3, 0)?);
        assert_eq!(rebooted.ringer.running_until(), None);
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
        resting.sleep.window.dark(clock.now()?);
        let step = resting.rest(&clock, &ringer()?, &lit());
        assert_eq!(
            step,
            Minute {
                log: Vec::new(),
                wait_seconds: 30,
            }
        );
        assert!(!resting.sleep.window.running());
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
    fn on_the_cable_after_the_window_the_rtc_is_armed_for_the_next_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = armed_at(&clock)?;
        let mut resting = Resting::new();
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        resting.rest(&clock, &ringer, &dark());
        assert_eq!(ringer.next(), Some(at(31, 0, 0).now()?));
        assert_eq!(resting.armed(), Some([ringer.next()].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn with_the_toggle_off_the_window_end_stays_suspended_for_the_next_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut ringer = armed_at(&clock)?;
        ringer.set_power_off(false);
        let mut resting = Resting::unplugged();
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer, &dark());
        let next = at(31, 0, 0).now()?;
        assert_eq!(
            step.log,
            [kmsg::line(&format!(
                "resumed from a suspend armed for {}",
                next.0
            ))]
        );
        assert_eq!(resting.offs(), Some([].as_slice()));
        assert_eq!(resting.armed(), Some([Some(next)].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn with_the_toggle_off_and_no_alarm_it_stays_suspended_with_the_rtc_disarmed()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut ringer = ringer()?;
        ringer.set_power_off(false);
        let mut resting = Resting::unplugged();
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer, &dark());
        assert_eq!(
            step.log,
            [kmsg::line("resumed from a suspend with the RTC disarmed")]
        );
        assert_eq!(resting.offs(), Some([].as_slice()));
        assert_eq!(resting.armed(), Some([None].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn with_the_toggle_off_an_unread_cable_is_not_a_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut ringer = ringer()?;
        ringer.set_power_off(false);
        let mut resting = Resting::unplugged();
        resting.cable_in = Err(HardwareError::Absent("Battery"));
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer, &dark());
        assert_eq!(
            step.log,
            [kmsg::line("resumed from a suspend with the RTC disarmed")]
        );
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn on_the_cable_after_the_window_with_no_alarm_the_rtc_is_disarmed()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut resting = Resting::new();
        resting.sleep.window.dark(at(19, 35, 0).now()?);
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
        resting.sleep.wake = Ok(FakeRtc {
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
        resting.sleep.suspend = Ok(FakeSuspend {
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
        resting.sleep.wake = Err(HardwareError::Absent("rtc0/wakealarm"));
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
        assert!(!resting.sleep.window.running());
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }

    #[test]
    fn off_the_cable_the_window_end_powers_off_two_minutes_before_the_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = armed_at(&clock)?;
        let mut resting = Resting::unplugged();
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer, &dark());
        let on_at = at(30, 58, 0).now()?;
        assert_eq!(
            step.log,
            [kmsg::line(&format!(
                "powered off, on again at Some({})",
                on_at.0
            ))]
        );
        assert_eq!(resting.offs(), Some([Some(on_at)].as_slice()));
        assert_eq!(resting.armed(), Some([].as_slice()));
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }

    #[test]
    fn off_the_cable_with_no_alarm_it_powers_off_with_none()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut resting = Resting::unplugged();
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        resting.rest(&at(19, 37, 30), &ringer()?, &dark());
        assert_eq!(resting.offs(), Some([None].as_slice()));
        assert_eq!(resting.suspends(), Some(0));
        Ok(())
    }

    #[test]
    fn off_the_cable_the_window_itself_still_suspends() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut resting = Resting::unplugged();
        resting.rest(&clock, &armed_at(&clock)?, &dark());
        assert_eq!(resting.offs(), Some([].as_slice()));
        assert_eq!(
            resting.armed(),
            Some([Some(after(&clock, WINDOW_SECONDS)?)].as_slice())
        );
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn a_failed_power_off_is_logged_and_suspends_until_the_alarm()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = armed_at(&clock)?;
        let mut resting = Resting::unplugged();
        resting.sleep.power = Ok(FakePower {
            offs: Vec::new(),
            failing: true,
        });
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer, &dark());
        assert_eq!(
            step.log[0],
            kmsg::failure("powering off", &refused("reboot"))
        );
        assert_eq!(resting.armed(), Some([ringer.next()].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn an_absent_power_off_suspends_all_the_same() -> Result<(), Box<dyn std::error::Error>> {
        let mut resting = Resting::unplugged();
        resting.sleep.power = Err(HardwareError::Absent("poweroff alarm"));
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&at(19, 37, 30), &ringer()?, &dark());
        assert_eq!(
            step.log[0],
            kmsg::failure("powering off", &HardwareError::Absent("poweroff alarm"))
        );
        assert_eq!(resting.armed(), Some([None].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn an_unread_cable_stays_suspended_rather_than_powering_off()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = armed_at(&clock)?;
        let mut resting = Resting::new();
        resting.cable_in = Err(HardwareError::Absent("Battery"));
        resting.sleep.window.dark(at(19, 35, 0).now()?);
        let step = resting.rest(&clock, &ringer, &dark());
        assert_eq!(
            step.log[0],
            kmsg::failure("reading the cable", &HardwareError::Absent("Battery"))
        );
        assert_eq!(resting.offs(), Some([].as_slice()));
        assert_eq!(resting.armed(), Some([ringer.next()].as_slice()));
        Ok(())
    }

    #[test]
    fn the_cable_is_read_from_the_charge_status() {
        assert_eq!(cable_in(&battery(55, ChargeStatus::Discharging)), Ok(false));
        assert_eq!(cable_in(&battery(55, ChargeStatus::Charging)), Ok(true));
        assert_eq!(cable_in(&battery(80, ChargeStatus::NotCharging)), Ok(true));
        let absent: Result<FakeBattery, HardwareError> = Err(HardwareError::Absent("Battery"));
        assert_eq!(cable_in(&absent), Err(HardwareError::Absent("Battery")));
    }

    #[test]
    fn a_boot_by_the_alarm_starts_dark() {
        let booted = booted(&Ok(FakeBoot(Ok(Boot::Alarm))), &at(6, 58, 13), LIT_SECONDS);
        assert_eq!(booted.light, Light::dark(LIT_SECONDS));
        assert_eq!(booted.log, [kmsg::line("booted by Alarm")]);
    }

    #[test]
    fn any_other_boot_shows_the_time() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        for boot in [Boot::Press, Boot::Charger, Boot::Other] {
            let booted = booted(&Ok(FakeBoot(Ok(boot))), &clock, LIT_SECONDS);
            assert_eq!(booted.light, Light::lit_at(clock.now()?, LIT_SECONDS));
        }
        Ok(())
    }

    #[test]
    fn the_boot_lights_the_panel_for_the_restored_lit_time()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let booted = booted(&Ok(FakeBoot(Ok(Boot::Press))), &clock, 120);
        assert_eq!(booted.light.seconds_left(clock.now()?), Some(120));
        Ok(())
    }

    #[test]
    fn an_unread_boot_reason_shows_the_time_and_says_so() -> Result<(), Box<dyn std::error::Error>>
    {
        let clock = at(19, 37, 30);
        let error = HardwareError::Absent("proc/cmdline");
        let from_adapter = booted(&Ok(FakeBoot(Err(error.clone()))), &clock, LIT_SECONDS);
        let unopened = booted(&Err::<FakeBoot, _>(error.clone()), &clock, LIT_SECONDS);
        for booted in [from_adapter, unopened] {
            assert_eq!(booted.light, Light::lit_at(clock.now()?, LIT_SECONDS));
            assert_eq!(
                booted.log,
                [kmsg::failure("reading the boot reason", &error)]
            );
        }
        Ok(())
    }

    #[test]
    fn a_failing_clock_at_boot_starts_dark() {
        let booted = booted(
            &Ok(FakeBoot(Ok(Boot::Press))),
            &FixedClock(Err(HardwareError::Absent("realtime clock"))),
            LIT_SECONDS,
        );
        assert_eq!(booted.light, Light::dark(LIT_SECONDS));
        assert_eq!(booted.log.len(), 2);
    }

    struct FakeSettings {
        record: [u8; RECORD_BYTES],
        saves: u32,
        failing: bool,
    }

    impl FakeSettings {
        fn blank() -> Self {
            Self {
                record: [0; RECORD_BYTES],
                saves: 0,
                failing: false,
            }
        }
    }

    impl Settings for FakeSettings {
        fn load(&mut self) -> Result<[u8; RECORD_BYTES], HardwareError> {
            Ok(self.record)
        }

        fn save(&mut self, slot: Slot, bytes: &[u8; SLOT_BYTES]) -> Result<(), HardwareError> {
            if self.failing {
                return Err(refused("reserved4"));
            }
            self.record[slot.offset()..slot.offset() + SLOT_BYTES].copy_from_slice(bytes);
            self.saves += 1;
            Ok(())
        }
    }

    fn berlin() -> Result<UtcOffset, TimeError> {
        UtcOffset::from_seconds(LOCAL_OFFSET_SECONDS)
    }

    fn boot(
        settings: &mut Result<FakeSettings, HardwareError>,
        clock: &FixedClock,
    ) -> Result<Restored, TimeError> {
        Ok(restored(
            settings,
            clock,
            TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?,
            berlin()?,
        ))
    }

    fn saves(settings: &Result<FakeSettings, HardwareError>) -> Option<u32> {
        settings.as_ref().ok().map(|settings| settings.saves)
    }

    #[test]
    fn a_blank_record_boots_the_default_alarm_disarmed_and_saves_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &at(19, 37, 30))?;
        assert_eq!(started.ringer, ringer()?);
        assert_eq!(started.saved.offset, berlin()?);
        assert_eq!(started.saved.window_seconds, WINDOW_SECONDS);
        assert!(started.saved.power_off);
        assert!(started.ringer.power_off());
        assert_eq!(started.saved.lit_seconds, LIT_SECONDS);
        assert_eq!(started.ringer.lit_seconds(), LIT_SECONDS);
        assert_eq!(started.log, [kmsg::line("settings never saved")]);
        let current = choices(&started.ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(persisted(&mut settings, &mut started.saved, current), None);
        assert_eq!(saves(&settings), Some(0));
        Ok(())
    }

    #[test]
    fn a_change_saves_once_and_the_next_boot_restores_it_armed()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let ringer = armed_at(&clock)?;
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current.clone()),
            Some(kmsg::line("settings saved"))
        );
        assert_eq!(persisted(&mut settings, &mut started.saved, current), None);
        assert_eq!(saves(&settings), Some(1));

        let later = at(20, 0, 0);
        let rebooted = boot(&mut settings, &later)?;
        assert_eq!(rebooted.log, [kmsg::line("settings loaded")]);
        assert!(rebooted.ringer.armed());
        assert!(rebooted.ringer.power_off());
        assert_eq!(
            rebooted.ringer.next(),
            Some(
                Alarm::once(TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?)
                    .next_after(later.now()?, berlin()?)
            )
        );
        Ok(())
    }

    #[test]
    fn a_turned_toggle_is_saved_and_survives_a_reboot() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let mut ringer = armed_at(&clock)?;
        ringer.set_power_off(false);
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert!(!current.power_off);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::line("settings saved"))
        );
        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert!(!rebooted.ringer.power_off());
        assert!(!rebooted.saved.power_off);
        assert!(rebooted.ringer.armed());
        Ok(())
    }

    #[test]
    fn a_stepped_lit_time_is_logged_saved_and_survives_a_reboot()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let mut ringer = started.ringer.clone();
        ringer.touched(
            Target::LitTime,
            Screen::Settings,
            0,
            clock.now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&started.ringer, &ringer),
            Some(kmsg::line(
                "alarms [07:00], active none, Quiet, power-off on, lit 30 s, timers [], running until none"
            ))
        );
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(current.lit_seconds, 30);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::line("settings saved"))
        );
        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.lit_seconds(), 30);
        assert_eq!(rebooted.saved.lit_seconds, 30);
        Ok(())
    }

    #[test]
    fn a_failed_save_is_logged_and_tried_again_at_the_next_panel_off()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        if let Ok(fake) = settings.as_mut() {
            fake.failing = true;
        }
        let current = choices(&armed_at(&clock)?, berlin()?, WINDOW_SECONDS);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current.clone()),
            Some(kmsg::failure("saving the settings", &refused("reserved4")))
        );
        if let Ok(fake) = settings.as_mut() {
            fake.failing = false;
        }
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::line("settings saved"))
        );
        assert_eq!(saves(&settings), Some(1));
        Ok(())
    }

    #[test]
    fn an_absent_partition_boots_the_default_and_logs_each_save()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings: Result<FakeSettings, HardwareError> =
            Err(HardwareError::Absent("reserved4"));
        let mut started = boot(&mut settings, &clock)?;
        assert_eq!(started.ringer, ringer()?);
        assert_eq!(
            started.log,
            [kmsg::failure(
                "loading the settings",
                &HardwareError::Absent("reserved4")
            )]
        );
        let current = choices(&armed_at(&clock)?, berlin()?, WINDOW_SECONDS);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::failure(
                "saving the settings",
                &HardwareError::Absent("reserved4")
            ))
        );
        Ok(())
    }

    #[test]
    fn an_unreadable_record_arms_nothing_and_a_clock_fault_does_not_overwrite_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut corrupt = FakeSettings::blank();
        corrupt.record[0] = 1;
        let mut settings = Ok(corrupt);
        let started = boot(&mut settings, &clock)?;
        assert!(!started.ringer.armed());
        assert!(started.log[0].contains("loading the settings"));

        let mut settings = Ok(FakeSettings::blank());
        let mut first = boot(&mut settings, &clock)?;
        let current = choices(&armed_at(&clock)?, berlin()?, WINDOW_SECONDS);
        persisted(&mut settings, &mut first.saved, current);
        let stopped = FixedClock(Err(HardwareError::Absent("realtime clock")));
        let mut faulted = boot(&mut settings, &stopped)?;
        assert!(!faulted.ringer.armed());
        assert_eq!(
            faulted.log[1],
            kmsg::failure(
                "arming the saved alarm",
                &HardwareError::Absent("realtime clock")
            )
        );
        let current = choices(&faulted.ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(persisted(&mut settings, &mut faulted.saved, current), None);
        assert!(boot(&mut settings, &clock)?.ringer.armed());
        Ok(())
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Step {
        Boot,
        RadioOff,
    }

    #[derive(Default)]
    struct FakeModem {
        failing: Option<Step>,
        done: Vec<Step>,
    }

    impl FakeModem {
        fn failing(step: Step) -> Self {
            Self {
                failing: Some(step),
                done: Vec::new(),
            }
        }

        fn step(&mut self, step: Step, device: &'static str) -> Result<(), HardwareError> {
            if self.failing == Some(step) {
                return Err(refused(device));
            }
            self.done.push(step);
            Ok(())
        }
    }

    impl Modem for FakeModem {
        fn boot(&mut self) -> Result<(), HardwareError> {
            self.step(Step::Boot, "balong_power")
        }

        fn radio_off(&mut self) -> Result<(), HardwareError> {
            self.step(Step::RadioOff, "appvcom")
        }
    }

    fn done(modem: &Result<FakeModem, HardwareError>) -> Option<&[Step]> {
        modem.as_ref().ok().map(|modem| modem.done.as_slice())
    }

    #[test]
    fn the_modem_boots_and_then_its_radio_goes_off() {
        let mut modem = Ok(FakeModem::default());
        assert_eq!(
            modem_booted(&mut modem),
            vec![kmsg::line("modem booted"), kmsg::line("modem radio off")]
        );
        assert_eq!(done(&modem), Some(&[Step::Boot, Step::RadioOff][..]));
    }

    #[test]
    fn a_failed_boot_is_logged_and_sends_no_radio_off() {
        let mut modem = Ok(FakeModem::failing(Step::Boot));
        assert_eq!(
            modem_booted(&mut modem),
            vec![kmsg::failure("booting the modem", &refused("balong_power"))]
        );
        assert_eq!(done(&modem), Some(&[][..]));
    }

    #[test]
    fn a_failed_radio_off_is_logged_after_the_boot() {
        let mut modem = Ok(FakeModem::failing(Step::RadioOff));
        assert_eq!(
            modem_booted(&mut modem),
            vec![
                kmsg::line("modem booted"),
                kmsg::failure("turning the modem's radio off", &refused("appvcom")),
            ]
        );
        assert_eq!(done(&modem), Some(&[Step::Boot][..]));
    }

    #[test]
    fn an_absent_modem_is_logged_and_nothing_more() {
        let mut modem: Result<FakeModem, HardwareError> = Err(HardwareError::Absent("his_modem"));
        assert_eq!(
            modem_booted(&mut modem),
            vec![kmsg::failure(
                "opening the modem",
                &HardwareError::Absent("his_modem")
            )]
        );
    }

    #[test]
    fn the_first_suspend_waits_for_the_modem_and_the_next_does_not()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let done = Arc::new(AtomicBool::new(false));
        let finished = Arc::clone(&done);
        let modem = std::thread::Builder::new().spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            finished.store(true, Ordering::SeqCst);
        })?;
        let mut suspend = AfterModem {
            suspend: FakeSuspend::default(),
            modem: Some(modem),
        };
        suspend.suspend()?;
        assert!(done.load(Ordering::SeqCst));
        assert!(suspend.modem.is_none());
        suspend.suspend()?;
        assert_eq!(suspend.suspend.suspends, 2);
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_pulses_the_vibrator_and_wakes_the_loop_for_the_next_pulse()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        assert_eq!(ringer.ring(), Ring::Ringing);
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(
            step,
            Minute {
                log: Vec::new(),
                wait_seconds: PERIOD_SECONDS.unsigned_abs(),
            }
        );
        ringer.pressed(Key::Power, clock.now()?);
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.wait_seconds, 10);
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!(vibrator.pulses, [PULSE_MILLISECONDS]);
        assert_eq!(vibrator.stops, 1);
        Ok(())
    }

    #[test]
    fn a_refusing_vibrator_is_logged_once_and_the_ring_goes_on()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let ringer = ringing_at(&clock)?;
        let mut buzz = buzz(Ok(FakeVibrator {
            refusing: true,
            ..FakeVibrator::default()
        }));
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log.len(), 1);
        assert_eq!(step.wait_seconds, 10);
        let step = pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        assert_eq!(ringer.ring(), Ring::Ringing);
        Ok(())
    }

    #[test]
    fn a_low_battery_off_the_cable_warns_until_the_cable_goes_in()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut lamp = lamp(Ok(FakeIndicator::default()));
        let unplugged = battery(20, ChargeStatus::Discharging);
        assert_eq!(warned(&unplugged, &mut lamp), Vec::<String>::new());
        assert_eq!(warned(&unplugged, &mut lamp), Vec::<String>::new());
        assert!(lamp.warning.shown());
        warned(&battery(20, ChargeStatus::Charging), &mut lamp);
        assert!(!lamp.warning.shown());
        let indicator = lamp.indicator.as_ref().map_err(Clone::clone)?;
        assert_eq!((indicator.warnings, indicator.clears), (1, 1));
        Ok(())
    }

    #[test]
    fn an_absent_battery_clears_the_warning() -> Result<(), Box<dyn std::error::Error>> {
        let mut lamp = lamp(Ok(FakeIndicator::default()));
        warned(&battery(10, ChargeStatus::Discharging), &mut lamp);
        let absent: Result<FakeBattery, HardwareError> = Err(HardwareError::Absent("battery"));
        warned(&absent, &mut lamp);
        assert!(!lamp.warning.shown());
        let indicator = lamp.indicator.as_ref().map_err(Clone::clone)?;
        assert_eq!((indicator.warnings, indicator.clears), (1, 1));
        Ok(())
    }

    #[test]
    fn a_refusing_indicator_is_logged_once() {
        let mut lamp = lamp(Ok(FakeIndicator {
            refusing: true,
            ..FakeIndicator::default()
        }));
        let low = battery(10, ChargeStatus::Discharging);
        assert_eq!(warned(&low, &mut lamp).len(), 1);
        assert_eq!(warned(&low, &mut lamp), Vec::<String>::new());
    }

    #[test]
    fn an_absent_indicator_logs_nothing() {
        let mut lamp = lamp(Err(HardwareError::Absent("indicator")));
        let low = battery(10, ChargeStatus::Discharging);
        assert_eq!(warned(&low, &mut lamp), Vec::<String>::new());
    }

    #[test]
    fn an_absent_vibrator_leaves_the_wait_alone() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let ringer = ringing_at(&clock)?;
        let mut buzz = buzz(Err(HardwareError::Absent("vibrator")));
        assert_eq!(
            pulse(&clock, &ringer, &mut buzz, 10),
            Minute {
                log: Vec::new(),
                wait_seconds: 10,
            }
        );
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_starts_the_speaker_once_and_a_dismissal_stops_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        pulse(&clock, &ringer, &mut buzz, 10);
        pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert!(buzz.sound.playing());
        ringer.pressed(Key::Power, clock.now()?);
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        assert!(!buzz.sound.playing());
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (1, 1));
        Ok(())
    }

    #[test]
    fn a_snooze_stops_the_speaker() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        pulse(&clock, &ringer, &mut buzz, 10);
        ringer.pressed(Key::VolumeUp, clock.now()?);
        assert!(matches!(ringer.ring(), Ring::Snoozed(_)));
        pulse(&clock, &ringer, &mut buzz, 10);
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (1, 1));
        Ok(())
    }

    #[test]
    fn a_refusing_speaker_is_logged_once_and_the_vibrator_still_pulses()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let ringer = ringing_at(&clock)?;
        let mut buzz = sounding(Ok(FakeSpeaker {
            refusing: true,
            ..FakeSpeaker::default()
        }));
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log.len(), 1);
        assert!(step.log[0].contains("driving the speaker"));
        let step = pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!(vibrator.pulses, [PULSE_MILLISECONDS; 2]);
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!(speaker.starts, 1);
        Ok(())
    }

    #[test]
    fn an_absent_speaker_leaves_the_vibrator_to_wake() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let ringer = ringing_at(&clock)?;
        let mut buzz = sounding(Err(HardwareError::Absent("speaker")));
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!(vibrator.pulses, [PULSE_MILLISECONDS]);
        Ok(())
    }
}
