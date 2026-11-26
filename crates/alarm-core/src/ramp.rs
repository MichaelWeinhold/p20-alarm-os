use crate::sound::Volume;
use crate::time::UnixSeconds;

pub const RAMP_CHOICES: [i64; 5] = [15, 30, 60, 120, 300];
pub const RAMP_OFF: i64 = 0;

#[must_use]
pub fn next_ramp_seconds(seconds: i64) -> i64 {
    RAMP_CHOICES
        .into_iter()
        .find(|&choice| choice > seconds)
        .unwrap_or(RAMP_OFF)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ramp {
    from: UnixSeconds,
    seconds: u64,
    to: Volume,
    volume: Volume,
}

impl Ramp {
    #[must_use]
    pub fn start(now: UnixSeconds, seconds: i64, to: Volume) -> Option<Self> {
        let seconds = u64::try_from(seconds).ok().filter(|&seconds| seconds > 0)?;
        Some(Self {
            from: now,
            seconds,
            to,
            volume: Volume::QUIETEST,
        })
    }

    #[must_use]
    pub fn volume(self) -> Volume {
        self.volume
    }

    pub fn tick(&mut self, now: UnixSeconds) {
        self.volume = (0..self.taken(now)).fold(Volume::QUIETEST, |volume, _| volume.up());
    }

    #[must_use]
    pub fn seconds_left(self, now: UnixSeconds) -> Option<u64> {
        let taken = self.taken(now);
        (taken < self.steps())
            .then(|| ((taken + 1) * self.seconds).div_ceil(self.steps()) - self.elapsed(now))
    }

    fn steps(self) -> u64 {
        u64::from(self.to.level().saturating_sub(Volume::QUIETEST.level()))
    }

    fn elapsed(self, now: UnixSeconds) -> u64 {
        u64::try_from(now.0.saturating_sub(self.from.0))
            .unwrap_or(0)
            .min(self.seconds)
    }

    fn taken(self, now: UnixSeconds) -> u64 {
        self.steps() * self.elapsed(now) / self.seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: UnixSeconds = UnixSeconds(1_794_512_278);

    fn at(seconds: u64) -> UnixSeconds {
        UnixSeconds(START.0 + i64::try_from(seconds).unwrap_or(i64::MAX - START.0))
    }

    fn levels() -> impl Iterator<Item = Volume> {
        (Volume::QUIETEST.level()..=Volume::LOUDEST.level()).filter_map(Volume::new)
    }

    #[test]
    fn a_tap_steps_through_every_length_and_back_to_off() {
        let mut seconds = RAMP_OFF;
        let mut seen = Vec::new();
        for _ in 0..=RAMP_CHOICES.len() {
            seconds = next_ramp_seconds(seconds);
            seen.push(seconds);
        }
        assert_eq!(seen, [15, 30, 60, 120, 300, RAMP_OFF]);
    }

    #[test]
    fn off_starts_no_ramp() {
        assert_eq!(Ramp::start(START, RAMP_OFF, Volume::LOUDEST), None);
        assert_eq!(Ramp::start(START, -15, Volume::LOUDEST), None);
    }

    #[test]
    fn a_ramp_starts_at_the_quietest_and_reaches_the_set_volume_when_its_length_has_passed()
    -> Result<(), &'static str> {
        for seconds in RAMP_CHOICES {
            for to in levels() {
                let mut ramp = Ramp::start(START, seconds, to).ok_or("no ramp")?;
                assert_eq!(ramp.volume(), Volume::QUIETEST);
                let length = seconds.unsigned_abs();
                let mut before = Volume::QUIETEST;
                for second in 0..length {
                    ramp.tick(at(second));
                    assert!(ramp.volume() >= before, "{seconds} {to:?} {second}");
                    assert!(ramp.volume() <= before.up(), "{seconds} {to:?} {second}");
                    assert!(
                        to == Volume::QUIETEST || ramp.volume() < to,
                        "{seconds} {to:?} {second}"
                    );
                    before = ramp.volume();
                }
                for second in [length, length + 1, 10 * length] {
                    ramp.tick(at(second));
                    assert_eq!(ramp.volume(), to, "{seconds} {to:?} {second}");
                    assert_eq!(ramp.seconds_left(at(second)), None);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn the_seconds_left_name_the_next_step() -> Result<(), &'static str> {
        for seconds in RAMP_CHOICES {
            for to in levels() {
                let mut ramp = Ramp::start(START, seconds, to).ok_or("no ramp")?;
                let mut now = 0;
                while let Some(left) = ramp.seconds_left(at(now)) {
                    let before = ramp.volume();
                    ramp.tick(at(now + left - 1));
                    assert_eq!(ramp.volume(), before, "{seconds} {to:?} {now}");
                    now += left;
                    ramp.tick(at(now));
                    assert_eq!(ramp.volume(), before.up(), "{seconds} {to:?} {now}");
                }
                assert_eq!(ramp.volume(), to, "{seconds} {to:?}");
                assert!(now <= seconds.unsigned_abs(), "{seconds} {to:?} {now}");
            }
        }
        Ok(())
    }

    #[test]
    fn a_clock_set_back_holds_the_ramp_at_its_start() -> Result<(), &'static str> {
        let mut ramp = Ramp::start(START, 30, Volume::LOUDEST).ok_or("no ramp")?;
        ramp.tick(UnixSeconds(START.0 - 60));
        assert_eq!(ramp.volume(), Volume::QUIETEST);
        assert_eq!(ramp.seconds_left(UnixSeconds(START.0 - 60)), Some(3));
        Ok(())
    }
}
