use alarm_core::alarm::Weekdays;
use alarm_core::battery::{ChargeLimit, ChargeStep, Gauge, stop_at_limit};
use std::thread::JoinHandle;

use alarm_core::hardware::{
    Battery, BootReason, Clock, Display, HardwareError, Indicator, Modem, PowerOff, Recovery,
    Settings, Sounds, Speaker, Suspend, Vibrator, WakeAlarm,
};
use alarm_core::light::Light;
use alarm_core::ringer::{Ring, Ringer, show};
use alarm_core::settings::{self, Choices};
#[cfg(test)]
use alarm_core::sound::Volume;
use alarm_core::sound::{Sound, SoundName};
use alarm_core::time::{
    Countdown, TimeOfDay, UnixSeconds, UtcOffset, Weekday, seconds_to_next_minute,
};
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
pub const SUSPEND_LEAD_SECONDS: i64 = 5;
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
            [light.seconds_left(now), light.volume_left(now), timer]
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
    let only = |on: bool| if on { ring } else { Ring::Quiet };
    if let Ok(speaker) = &mut buzz.speaker {
        match (buzz.sound.choose(ringer.sound(), speaker), ringer.sound()) {
            (Some(Ok(())), Some(sound)) => {
                log.push(kmsg::line(&format!(
                    "alarm sound {} loaded",
                    sound.as_str()
                )));
            }
            (Some(Ok(())), None) => log.push(kmsg::line("no alarm sound chosen")),
            (Some(Err(error)), _) => log.push(kmsg::failure("loading the alarm sound", &error)),
            (None, _) => {}
        }
        if let Err(error) = buzz
            .sound
            .tick(only(ringer.outputs().sound), ringer.volume(), speaker)
        {
            log.push(kmsg::failure("driving the speaker", &error));
        }
    }
    let (Ok(vibrator), Ok(now)) = (&mut buzz.vibrator, clock.now()) else {
        return Minute { log, wait_seconds };
    };
    if let Err(error) = buzz
        .vibration
        .tick(now, only(ringer.outputs().vibration), vibrator)
    {
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
    Waiting(u64),
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
    light: &Light,
    cable_in: Result<bool, HardwareError>,
    sleep: &mut Sleep<impl WakeAlarm, impl Suspend, impl PowerOff>,
    log: &mut Vec<String>,
) -> Result<Rest, HardwareError> {
    let now = clock.now()?;
    sleep.window.dark(now);
    if let Some(left) = light.settling(now) {
        return Ok(Rest::Waiting(left));
    }
    let until = match sleep.window.answer(now, ringer.held(), ringer.next()) {
        Answer::Suspend(at) if at.0 > now.0.saturating_add(SUSPEND_LEAD_SECONDS) => Some(at),
        Answer::Suspend(at) if at > now => {
            return Ok(Rest::Waiting(at.0.saturating_sub(now.0).unsigned_abs()));
        }
        Answer::Hold => None,
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
    let (line, wait_seconds) = match suspended(clock, ringer, light, cable_in, sleep, &mut log) {
        Ok(Rest::Awake) => (None, wait_seconds),
        Ok(Rest::Waiting(left)) => (None, wait_seconds.min(left)),
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
        alarms: ringer.alarms().to_vec(),
        timers: ringer.timers().to_vec(),
        lit_seconds: ringer.lit_seconds(),
        volume: ringer.volume(),
        outputs: ringer.outputs(),
        sound: ringer.sound().cloned(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    pub saved: Choices,
    pub ringer: Ringer,
    pub log: Vec<String>,
}

#[must_use]
pub fn sounds_listed(sounds: &impl Sounds, ringer: &mut Ringer) -> Vec<String> {
    match sounds.list() {
        Ok(listed) => {
            let line = kmsg::line(&format!("{} alarm sounds listed", listed.len()));
            ringer.set_sounds(listed);
            vec![line]
        }
        Err(error) => vec![kmsg::failure("listing the alarm sounds", &error)],
    }
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
    let mut ringer = Ringer::listed(loaded.alarms.clone());
    ringer.set_power_off(loaded.power_off);
    ringer.set_lit_seconds(loaded.lit_seconds);
    ringer.set_volume(loaded.volume);
    ringer.set_outputs(loaded.outputs);
    ringer.set_sound(loaded.sound.clone());
    ringer.set_timers(loaded.timers.clone());
    if ringer.alarms().iter().any(|set| set.armed) {
        match clock.now() {
            Ok(now) => ringer.rearm(now, loaded.offset),
            Err(error) => log.push(kmsg::failure("arming the saved alarms", &error)),
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

pub fn recover(
    settings: &mut Result<impl Settings, HardwareError>,
    saved: &mut Choices,
    current: Choices,
    recovery: &mut impl Recovery,
    log: &mut impl FnMut(&str),
) {
    if let Some(line) = persisted(settings, saved, current) {
        log(&line);
    }
    log(&kmsg::line("rebooting into recovery"));
    if let Err(error) = recovery.reboot_into_recovery() {
        log(&kmsg::failure("rebooting into recovery", &error));
    }
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

fn weekdays(repeat: Weekdays) -> String {
    if repeat.is_empty() {
        return "once".to_owned();
    }
    Weekday::ALL
        .into_iter()
        .zip("MTWTFSS".chars())
        .map(|(day, letter)| if repeat.contains(day) { letter } else { '-' })
        .collect()
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
            ringer.volume(),
            ringer.outputs(),
            ringer.sound().cloned(),
            ringer.timers().to_vec(),
            ringer.running_until(),
            ringer.paused(),
        )
    };
    let clock = |time: TimeOfDay| format!("{:02}:{:02}", time.hour(), time.minute());
    let length = |timer: Countdown| {
        format!(
            "{:02}:{:02}:{:02}",
            timer.hours(),
            timer.minutes(),
            timer.seconds()
        )
    };
    let switched = |on: bool| if on { "on" } else { "off" };
    (state(before) != state(after)).then(|| {
        let alarms: Vec<String> = after
            .alarms()
            .iter()
            .map(|set| {
                format!(
                    "{} {} {}",
                    clock(set.alarm.time),
                    weekdays(set.alarm.repeat),
                    switched(set.armed)
                )
            })
            .collect();
        let timers: Vec<String> = after.timers().iter().map(|&timer| length(timer)).collect();
        let next = after
            .next()
            .map_or_else(|| "none".to_owned(), |next| next.0.to_string());
        let running = match (after.running_until(), after.paused()) {
            (Some(ends), _) => format!("until {}", ends.0),
            (None, Some(left)) => format!("paused {}", length(left)),
            (None, None) => "none".to_owned(),
        };
        kmsg::line(&format!(
            "alarms [{}], next {next}, {:?}, power-off {}, lit {} s, volume {}, sound {} {}, vibration {}, timers [{}], running {running}",
            alarms.join(", "),
            after.ring(),
            switched(after.power_off()),
            after.lit_seconds(),
            after.volume().level(),
            switched(after.outputs().sound),
            after.sound().map_or("none", SoundName::as_str),
            switched(after.outputs().vibration),
            timers.join(", ")
        ))
    })
}

#[cfg(test)]
mod tests {
    use alarm_core::alarm::Alarm;
    use alarm_core::battery::{BatteryReading, ChargeStatus, Health, Percent};
    use alarm_core::boot::Boot;
    use alarm_core::face::{
        BACKGROUND, Face, Field, Frame, Listed, Row, Screen, Size, Target, WARNING, render,
    };
    use alarm_core::hardware::Sounds;
    use alarm_core::input::{InputEvent, Key};
    use alarm_core::light::LIT_SECONDS;
    use alarm_core::ringer::Outputs;
    use alarm_core::settings::{RECORD_BYTES, SLOT_BYTES, Slot};
    use alarm_core::time::TimeError;
    use alarm_core::vibration::{PERIOD_SECONDS, PULSE_MILLISECONDS};

    use super::*;

    const SIZE: Size = Size {
        width: 108,
        height: 228,
    };
    const SILENT: Outputs = Outputs {
        sound: false,
        vibration: false,
    };

    fn haloed(frame: &Frame) -> bool {
        (SIZE.height * 19 / 20..SIZE.height)
            .all(|y| frame.row(y).iter().all(|&colour| colour == BACKGROUND))
    }

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
        loads: Vec<String>,
        starts: u32,
        stops: u32,
        volumes: Vec<Volume>,
        refusing: bool,
        missing: bool,
    }

    impl Speaker for FakeSpeaker {
        fn load(&mut self, sound: &SoundName) -> Result<(), HardwareError> {
            self.loads.push(sound.as_str().to_owned());
            if self.missing {
                Err(HardwareError::Absent("alarm sound"))
            } else {
                Ok(())
            }
        }

        fn start(&mut self, volume: Volume) -> Result<(), HardwareError> {
            self.starts += 1;
            self.volumes.push(volume);
            if self.refusing {
                Err(refused("speaker"))
            } else {
                Ok(())
            }
        }

        fn set_volume(&mut self, volume: Volume) -> Result<(), HardwareError> {
            self.volumes.push(volume);
            Ok(())
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

    fn ringtone() -> Option<SoundName> {
        SoundName::new(settings::PLAYED_BEFORE)
    }

    fn buzz(vibrator: Result<FakeVibrator, HardwareError>) -> Buzz<FakeVibrator, FakeSpeaker> {
        Buzz {
            vibrator,
            ..sounding(Ok(FakeSpeaker::default()))
        }
    }

    fn sounding(
        mut speaker: Result<FakeSpeaker, HardwareError>,
    ) -> Buzz<FakeVibrator, FakeSpeaker> {
        let mut sound = Sound::quiet();
        if let Ok(speaker) = &mut speaker {
            let _ = sound.choose(ringtone().as_ref(), speaker);
            speaker.loads.clear();
        }
        Buzz {
            vibration: Vibration::still(),
            vibrator: Ok(FakeVibrator::default()),
            sound,
            speaker,
        }
    }

    fn ringing_at(clock: &FixedClock) -> Result<Ringer, Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        ringer.set_sound(ringtone());
        let before = UnixSeconds(clock.now()?.0 - 60);
        ringer.touched(Target::Switch(0), Screen::Alarm, 0, before, UtcOffset::UTC);
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
        ringer.set_timers(vec![Countdown::new(0, 0, seconds)?]);
        ringer.touched(
            Target::Play(0),
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
            Target::Switch(0),
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
            weekday: Weekday::Wednesday,
            day: 11,
            alarms: vec![Listed {
                alarm: Alarm {
                    time: TimeOfDay::new(ALARM_HOUR, ALARM_MINUTE)?,
                    repeat: Weekdays::EVERY_DAY,
                },
                armed: false,
                until: None,
            }],
            until: None,
            editing: None,
            rang: None,
            marked: None,
            ringing: false,
            battery,
            screen: Screen::Alarm,
            power_off: true,
            lit_seconds: LIT_SECONDS,
            outputs: Outputs::BOTH,
            recovery_marked: false,
            timers: Vec::new(),
            countdown: Countdown::default(),
            running: None,
            rung: None,
            volume: None,
            sound: None,
            listing: None,
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
    fn an_armed_alarm_shows_the_halo_at_its_minute() -> Result<(), Box<dyn std::error::Error>> {
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let mut ringer = ringer()?;
        let before = at(6, 59, 0);
        ringer.touched(
            Target::Switch(0),
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
        assert_eq!(
            display.shown.iter().map(haloed).collect::<Vec<_>>(),
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
            Target::Switch(0),
            Screen::Alarm,
            0,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&before, &after),
            Some(kmsg::line(&format!(
                "alarms [07:00 MTWTFSS on], next {}, Quiet, power-off on, lit 10 s, volume 15, sound on none, vibration on, timers [], running none",
                at(7, 0, 0).now()?.0
            )))
        );
        Ok(())
    }

    #[test]
    fn a_turned_toggle_is_logged() -> Result<(), Box<dyn std::error::Error>> {
        let before = ringer()?;
        let mut after = before.clone();
        after.touched(
            Target::PowerOff,
            Screen::Settings,
            0,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&before, &after),
            Some(kmsg::line(
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off off, lit 10 s, volume 15, sound on none, vibration on, timers [], running none"
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
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 10 s, volume 15, sound on none, vibration on, timers [00:00:45], running until {}",
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

    fn paused_at(clock: &FixedClock, seconds: u8) -> Result<Ringer, Box<dyn std::error::Error>> {
        let mut ringer = running_at(clock, seconds)?;
        ringer.touched(
            Target::Pause,
            Screen::Timer,
            0,
            clock.now()?,
            UtcOffset::UTC,
        );
        Ok(ringer)
    }

    #[test]
    fn a_paused_timer_is_logged_with_its_time_left() -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(6, 0, 0);
        let running = running_at(&clock, 45)?;
        let paused = paused_at(&clock, 45)?;
        assert_eq!(
            change(&running, &paused),
            Some(kmsg::line(
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 10 s, volume 15, sound on none, vibration on, timers [00:00:45], running paused 00:00:45"
            ))
        );
        Ok(())
    }

    #[test]
    fn a_timer_of_an_hour_or_more_is_logged_with_its_hours()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(6, 0, 0);
        let mut running = ringer()?;
        running.set_timers(vec![Countdown::new(1, 0, 5)?]);
        running.touched(
            Target::Play(0),
            Screen::Timer,
            0,
            clock.now()?,
            UtcOffset::UTC,
        );
        let mut paused = running.clone();
        paused.touched(
            Target::Pause,
            Screen::Timer,
            0,
            clock.now()?,
            UtcOffset::UTC,
        );
        assert_eq!(
            change(&running, &paused),
            Some(kmsg::line(
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 10 s, volume 15, sound on none, vibration on, timers [01:00:05], running paused 01:00:05"
            ))
        );
        Ok(())
    }

    #[test]
    fn a_paused_timer_holds_the_phone_suspended_past_the_window_with_the_rtc_disarmed()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let ringer = paused_at(&clock, 50)?;
        assert_eq!(ringer.next(), None);
        let mut resting = Resting::unplugged();
        resting.sleep.window.dark(at(19, 30, 0).now()?);
        resting.rest(&clock, &ringer, &dark());
        assert_eq!(resting.offs(), Some([].as_slice()));
        assert_eq!(resting.armed(), Some([None].as_slice()));
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn an_alarm_edited_but_not_done_is_not_logged() -> Result<(), Box<dyn std::error::Error>> {
        let before = ringer()?;
        let mut after = before.clone();
        for target in [Target::Entry(0), Target::Day(Weekday::Monday)] {
            after.touched(target, Screen::Alarm, 0, at(6, 0, 0).now()?, UtcOffset::UTC);
            after.lifted();
        }
        after.touched(
            Target::Wheel(Field::Hours, Row::Picked),
            Screen::Alarm,
            500,
            at(6, 0, 0).now()?,
            UtcOffset::UTC,
        );
        after.dragged(400, 46);
        assert_ne!(after.editing(), None);
        assert_eq!(change(&before, &after), None);
        Ok(())
    }

    #[test]
    fn every_saved_alarm_is_restored_with_its_days_and_switch_and_the_soonest_rings_next()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let mut ringer = started.ringer.clone();
        for target in [
            Target::Add,
            Target::Wheel(Field::Hours, Row::Above),
            Target::Done,
            Target::Add,
            Target::Wheel(Field::Hours, Row::Above),
            Target::Wheel(Field::Hours, Row::Above),
            Target::Day(Weekday::Thursday),
            Target::Done,
            Target::Switch(0),
        ] {
            ringer.touched(target, Screen::Alarm, 0, clock.now()?, berlin()?);
            ringer.lifted();
        }
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        let shown: Vec<_> = current
            .alarms
            .iter()
            .map(|set| (set.alarm.time.hour(), set.alarm.repeat, set.armed))
            .collect();
        assert_eq!(
            shown,
            [
                (7, Weekdays::EVERY_DAY, true),
                (21, Weekdays::NONE, true),
                (22, Weekdays::NONE.with(Weekday::Thursday), true),
            ]
        );
        persisted(&mut settings, &mut started.saved, current);

        let later = at(19, 40, 0);
        let rebooted = boot(&mut settings, &later)?;
        assert_eq!(rebooted.ringer.alarms(), ringer.alarms());
        assert_eq!(rebooted.ringer.timers(), []);
        assert_eq!(
            rebooted.ringer.next(),
            Some(Alarm::once(TimeOfDay::new(21, 37)?).next_after(later.now()?, berlin()?))
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
        ringer.set_timers(vec![Countdown::new(0, 12, 5)?, Countdown::new(0, 3, 0)?]);
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(current.timers, ringer.timers());
        persisted(&mut settings, &mut started.saved, current);

        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.timers(), ringer.timers());
        assert_eq!(rebooted.ringer.countdown(), Countdown::new(0, 3, 0)?);
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
    fn a_press_that_darkens_the_panel_suspends_only_once_power_is_quiet()
    -> Result<(), Box<dyn std::error::Error>> {
        let ringer = ringer()?;
        let mut resting = Resting::new();
        let mut light = Light::lit_at(at(19, 37, 0).now()?, LIT_SECONDS);
        light.heard(
            InputEvent::Press(Key::Power),
            Ring::Quiet,
            at(19, 37, 30).now()?,
        );
        let pressed = resting.rest(&at(19, 37, 30), &ringer, &light);
        let settling = resting.rest(&at(19, 37, 31), &ringer, &light);
        assert_eq!(resting.suspends(), Some(0));
        assert_eq!(
            (pressed.wait_seconds, settling.wait_seconds),
            (2, 1),
            "{pressed:?} {settling:?}"
        );
        resting.rest(&at(19, 37, 32), &ringer, &light);
        assert_eq!(resting.suspends(), Some(1));
        Ok(())
    }

    #[test]
    fn a_wake_due_within_the_suspend_lead_stays_awake_until_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut resting = Resting::new();
        let soon = resting.rest(&clock, &running_at(&clock, 3)?, &dark());
        assert_eq!(resting.suspends(), Some(0));
        assert_eq!(soon.wait_seconds, 3);
        resting.rest(&clock, &running_at(&clock, 6)?, &dark());
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

    #[derive(Debug, Default)]
    struct FakeRecovery {
        reboots: u32,
        failing: bool,
    }

    impl Recovery for FakeRecovery {
        fn reboot_into_recovery(&mut self) -> Result<(), HardwareError> {
            self.reboots += 1;
            if self.failing {
                return Err(refused("reboot"));
            }
            Ok(())
        }
    }

    #[test]
    fn a_changed_setting_is_saved_and_logged_before_the_reboot_into_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let current = choices(&armed_at(&clock)?, berlin()?, WINDOW_SECONDS);
        let mut recovery = FakeRecovery::default();
        let mut log = Vec::new();
        recover(
            &mut settings,
            &mut started.saved,
            current,
            &mut recovery,
            &mut |line| log.push(line.to_owned()),
        );
        assert_eq!(
            log,
            [
                kmsg::line("settings saved"),
                kmsg::line("rebooting into recovery")
            ]
        );
        assert_eq!(saves(&settings), Some(1));
        assert_eq!(recovery.reboots, 1);
        assert!(boot(&mut settings, &at(20, 0, 0))?.ringer.armed());
        Ok(())
    }

    #[test]
    fn unchanged_settings_save_nothing_and_a_failed_reboot_into_recovery_is_logged()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        let current = started.saved.clone();
        let mut recovery = FakeRecovery {
            failing: true,
            ..FakeRecovery::default()
        };
        let mut log = Vec::new();
        recover(
            &mut settings,
            &mut started.saved,
            current,
            &mut recovery,
            &mut |line| log.push(line.to_owned()),
        );
        assert_eq!(
            log,
            [
                kmsg::line("rebooting into recovery"),
                kmsg::failure("rebooting into recovery", &refused("reboot"))
            ]
        );
        assert_eq!(saves(&settings), Some(0));
        assert_eq!(recovery.reboots, 1);
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
    fn a_turned_sound_and_vibration_are_logged_saved_and_survive_a_reboot()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        assert_eq!(started.ringer.outputs(), Outputs::BOTH);
        let mut ringer = started.ringer.clone();
        for target in [Target::Sound, Target::Vibration] {
            ringer.touched(target, Screen::Settings, 0, clock.now()?, UtcOffset::UTC);
        }
        assert_eq!(
            change(&started.ringer, &ringer),
            Some(kmsg::line(
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 10 s, volume 15, sound off none, vibration off, timers [], running none"
            ))
        );
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(current.outputs, SILENT);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::line("settings saved"))
        );
        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.outputs(), SILENT);
        assert_eq!(rebooted.saved.outputs, SILENT);
        Ok(())
    }

    #[test]
    fn a_chosen_sound_is_logged_saved_and_survives_a_reboot()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        assert_eq!(started.ringer.sound(), None);
        let carbon = SoundName::new("Carbon.wav").ok_or("no name")?;
        let mut ringer = started.ringer.clone();
        ringer.set_sound(Some(carbon.clone()));
        assert_eq!(
            change(&started.ringer, &ringer),
            Some(kmsg::line(
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 10 s, volume 15, sound on Carbon.wav, vibration on, timers [], running none"
            ))
        );
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::line("settings saved"))
        );
        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.sound(), Some(&carbon));
        assert_eq!(rebooted.saved.sound, Some(carbon));
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
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 30 s, volume 15, sound on none, vibration on, timers [], running none"
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
    fn a_stepped_volume_is_logged_saved_and_survives_a_reboot()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(19, 37, 30);
        let mut settings = Ok(FakeSettings::blank());
        let mut started = boot(&mut settings, &clock)?;
        assert_eq!(started.ringer.volume(), Volume::LOUDEST);
        let mut ringer = started.ringer.clone();
        ringer.pressed(Key::VolumeDown);
        ringer.pressed(Key::VolumeDown);
        assert_eq!(
            change(&started.ringer, &ringer),
            Some(kmsg::line(
                "alarms [07:00 MTWTFSS off], next none, Quiet, power-off on, lit 10 s, volume 13, sound on none, vibration on, timers [], running none"
            ))
        );
        let current = choices(&ringer, berlin()?, WINDOW_SECONDS);
        assert_eq!(
            persisted(&mut settings, &mut started.saved, current),
            Some(kmsg::line("settings saved"))
        );
        let rebooted = boot(&mut settings, &at(20, 0, 0))?;
        assert_eq!(rebooted.ringer.volume().level(), 13);
        assert_eq!(rebooted.saved.volume.level(), 13);
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
                "arming the saved alarms",
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
        ringer.pressed(Key::Power);
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
        ringer.pressed(Key::Power);
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        assert!(!buzz.sound.playing());
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (1, 1));
        Ok(())
    }

    #[test]
    fn a_volume_key_turns_the_ringing_speaker_down_and_leaves_the_vibrator_on()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        pulse(&clock, &ringer, &mut buzz, 10);
        ringer.pressed(Key::VolumeUp);
        ringer.pressed(Key::VolumeDown);
        assert_eq!(ringer.ring(), Ring::Ringing);
        pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert!(buzz.sound.playing());
        assert!(buzz.vibration.pulsing());
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (1, 0));
        assert_eq!(speaker.volumes, [Volume::LOUDEST, Volume::LOUDEST.down()]);
        Ok(())
    }

    #[test]
    fn a_ring_with_the_sound_off_starts_no_speaker_and_the_vibrator_still_pulses()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        ringer.set_outputs(Outputs {
            sound: false,
            ..Outputs::BOTH
        });
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        pulse(&clock, &ringer, &mut buzz, 10);
        pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert!(!buzz.sound.playing());
        assert!(buzz.vibration.pulsing());
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (0, 0));
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!(vibrator.pulses, [PULSE_MILLISECONDS; 2]);
        Ok(())
    }

    #[test]
    fn a_ring_with_the_vibration_off_starts_no_vibrator_and_the_speaker_still_plays()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        ringer.set_outputs(Outputs {
            vibration: false,
            ..Outputs::BOTH
        });
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.wait_seconds, 10);
        pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert!(buzz.sound.playing());
        assert!(!buzz.vibration.pulsing());
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (1, 0));
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!((vibrator.pulses.len(), vibrator.stops), (0, 0));
        Ok(())
    }

    #[test]
    fn a_ring_with_both_off_shows_the_halo_starts_neither_and_power_ends_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut battery = battery(55, ChargeStatus::Discharging);
        let mut display = FakeDisplay::default();
        let mut ringer = ringing_at(&clock)?;
        ringer.set_outputs(SILENT);
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        let mut light = lit();
        minute(
            &clock,
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut light,
            &mut display,
        );
        pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(ringer.ring(), Ring::Ringing);
        ringer.pressed(Key::Power);
        assert_eq!(ringer.ring(), Ring::Quiet);
        minute(
            &at(7, 0, 2),
            UtcOffset::UTC,
            &mut battery,
            &mut ringer,
            &mut light,
            &mut display,
        );
        pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert_eq!(
            display.shown.iter().map(haloed).collect::<Vec<_>>(),
            [true, false]
        );
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (0, 0));
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!((vibrator.pulses.len(), vibrator.stops), (0, 0));
        Ok(())
    }

    #[test]
    fn a_toggle_turned_during_a_ring_stops_or_starts_its_output_at_once()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        let mut buzz = buzz(Ok(FakeVibrator::default()));
        pulse(&clock, &ringer, &mut buzz, 10);
        ringer.set_outputs(SILENT);
        pulse(&at(7, 0, 1), &ringer, &mut buzz, 10);
        assert!(!buzz.sound.playing());
        assert!(!buzz.vibration.pulsing());
        ringer.set_outputs(Outputs::BOTH);
        pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert!(buzz.sound.playing());
        assert!(buzz.vibration.pulsing());
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!((speaker.starts, speaker.stops), (2, 1));
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!((vibrator.pulses.len(), vibrator.stops), (2, 1));
        Ok(())
    }

    fn unloaded(speaker: FakeSpeaker) -> Buzz<FakeVibrator, FakeSpeaker> {
        Buzz {
            sound: Sound::quiet(),
            speaker: Ok(speaker),
            ..buzz(Ok(FakeVibrator::default()))
        }
    }

    #[test]
    fn the_chosen_sound_is_loaded_and_logged_at_the_first_pulse_and_again_when_it_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(6, 0, 0);
        let mut ringer = ringer()?;
        ringer.set_sound(ringtone());
        let mut buzz = unloaded(FakeSpeaker::default());
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, [kmsg::line("alarm sound ringtone.wav loaded")]);
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        ringer.set_sound(SoundName::new("Carbon.wav"));
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, [kmsg::line("alarm sound Carbon.wav loaded")]);
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!(speaker.loads, ["ringtone.wav", "Carbon.wav"]);
        assert_eq!(speaker.starts, 0);
        Ok(())
    }

    struct FakeSounds(Result<Vec<SoundName>, HardwareError>);

    impl Sounds for FakeSounds {
        fn list(&self) -> Result<Vec<SoundName>, HardwareError> {
            self.0.clone()
        }
    }

    #[test]
    fn the_alarm_sounds_are_listed_into_the_ringer_and_counted_in_the_log()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        let listed = ["ringtone.wav", "Carbon.wav"]
            .into_iter()
            .filter_map(SoundName::new)
            .collect();
        let log = sounds_listed(&FakeSounds(Ok(listed)), &mut ringer);
        assert_eq!(log, [kmsg::line("2 alarm sounds listed")]);
        let shown: Vec<&str> = ringer.sounds().iter().map(SoundName::as_str).collect();
        assert_eq!(shown, ["Carbon.wav", "ringtone.wav"]);
        Ok(())
    }

    #[test]
    fn a_directory_that_does_not_list_is_logged_and_leaves_the_list_empty()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut ringer = ringer()?;
        let absent = HardwareError::Absent("alarm sound");
        let log = sounds_listed(&FakeSounds(Err(absent.clone())), &mut ringer);
        assert_eq!(log, [kmsg::failure("listing the alarm sounds", &absent)]);
        assert_eq!(ringer.sounds(), []);
        Ok(())
    }

    #[test]
    fn a_chosen_sound_that_does_not_open_rings_on_the_vibrator_alone()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let ringer = ringing_at(&clock)?;
        let mut buzz = unloaded(FakeSpeaker {
            missing: true,
            ..FakeSpeaker::default()
        });
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(
            step.log,
            [kmsg::failure(
                "loading the alarm sound",
                &HardwareError::Absent("alarm sound")
            )]
        );
        let step = pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!(vibrator.pulses, [PULSE_MILLISECONDS; 2]);
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!(speaker.loads, ["ringtone.wav"]);
        assert_eq!(speaker.starts, 0);
        Ok(())
    }

    #[test]
    fn no_sound_chosen_is_logged_once_and_rings_on_the_vibrator_alone()
    -> Result<(), Box<dyn std::error::Error>> {
        let clock = at(7, 0, 0);
        let mut ringer = ringing_at(&clock)?;
        ringer.set_sound(None);
        let mut buzz = unloaded(FakeSpeaker::default());
        let step = pulse(&clock, &ringer, &mut buzz, 10);
        assert_eq!(step.log, [kmsg::line("no alarm sound chosen")]);
        let step = pulse(&at(7, 0, 2), &ringer, &mut buzz, 10);
        assert_eq!(step.log, Vec::<String>::new());
        let vibrator = buzz.vibrator.as_ref().map_err(Clone::clone)?;
        assert_eq!(vibrator.pulses, [PULSE_MILLISECONDS; 2]);
        let speaker = buzz.speaker.as_ref().map_err(Clone::clone)?;
        assert_eq!(speaker.loads, Vec::<String>::new());
        assert_eq!(speaker.starts, 0);
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
