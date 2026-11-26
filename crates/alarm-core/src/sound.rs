use crate::hardware::{HardwareError, Speaker};
use crate::ringer::Ring;

pub const UNITY_GAIN: u32 = 1 << 16;
const STEP_GAIN: u32 = 46_396;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Volume(u8);

impl Volume {
    pub const QUIETEST: Self = Self(1);
    pub const LOUDEST: Self = Self(15);

    #[must_use]
    pub fn new(level: u8) -> Option<Self> {
        (Self::QUIETEST.0..=Self::LOUDEST.0)
            .contains(&level)
            .then_some(Self(level))
    }

    #[must_use]
    pub const fn level(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn up(self) -> Self {
        if self.0 < Self::LOUDEST.0 {
            Self(self.0 + 1)
        } else {
            Self::LOUDEST
        }
    }

    #[must_use]
    pub const fn down(self) -> Self {
        if self.0 > Self::QUIETEST.0 {
            Self(self.0 - 1)
        } else {
            Self::QUIETEST
        }
    }

    #[must_use]
    pub fn gain(self) -> u32 {
        (self.0..Self::LOUDEST.0).fold(UNITY_GAIN, |gain, _| {
            (gain * STEP_GAIN + UNITY_GAIN / 2) / UNITY_GAIN
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum State {
    #[default]
    Quiet,
    Playing(Volume),
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sound {
    state: State,
}

impl Sound {
    #[must_use]
    pub fn quiet() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn playing(&self) -> bool {
        matches!(self.state, State::Playing(_))
    }

    pub fn tick(
        &mut self,
        ring: Ring,
        volume: Volume,
        speaker: &mut impl Speaker,
    ) -> Result<(), HardwareError> {
        match (ring, self.state) {
            (Ring::Ringing, State::Playing(given)) if given != volume => {
                self.state = State::Playing(volume);
                speaker.set_volume(volume)
            }
            (Ring::Ringing, State::Playing(_) | State::Failed)
            | (Ring::Quiet | Ring::Held(_), State::Quiet) => Ok(()),
            (Ring::Ringing, State::Quiet) => match speaker.start(volume) {
                Ok(()) => {
                    self.state = State::Playing(volume);
                    Ok(())
                }
                Err(error) => {
                    self.state = State::Failed;
                    Err(error)
                }
            },
            (Ring::Quiet | Ring::Held(_), State::Playing(_)) => {
                self.state = State::Quiet;
                speaker.stop()
            }
            (Ring::Quiet | Ring::Held(_), State::Failed) => {
                self.state = State::Quiet;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::UnixSeconds;

    const HELD: Ring = Ring::Held(UnixSeconds(1_794_512_818));
    const LOUDEST: Volume = Volume::LOUDEST;
    const START: Call = Call::Start(Volume::LOUDEST);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Start(Volume),
        Volume(Volume),
        Stop,
    }

    #[derive(Default)]
    struct FakeSpeaker {
        calls: Vec<Call>,
        failing: bool,
    }

    impl FakeSpeaker {
        fn failing() -> Self {
            Self {
                calls: Vec::new(),
                failing: true,
            }
        }

        fn answer(&self) -> Result<(), HardwareError> {
            if self.failing {
                Err(HardwareError::Failed {
                    device: "speaker",
                    reason: "refused".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    impl Speaker for FakeSpeaker {
        fn start(&mut self, volume: Volume) -> Result<(), HardwareError> {
            self.calls.push(Call::Start(volume));
            self.answer()
        }

        fn set_volume(&mut self, volume: Volume) -> Result<(), HardwareError> {
            self.calls.push(Call::Volume(volume));
            self.answer()
        }

        fn stop(&mut self) -> Result<(), HardwareError> {
            self.calls.push(Call::Stop);
            self.answer()
        }
    }

    #[test]
    fn a_quiet_alarm_never_touches_the_speaker() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        sound.tick(Ring::Quiet, LOUDEST, &mut speaker)?;
        sound.tick(HELD, LOUDEST, &mut speaker)?;
        assert_eq!(speaker.calls, []);
        assert!(!sound.playing());
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_starts_the_sound_once() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert!(sound.playing());
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert_eq!(speaker.calls, [START]);
        Ok(())
    }

    #[test]
    fn a_dismissal_stops_the_sound_at_once() -> Result<(), HardwareError> {
        for quiet in [Ring::Quiet, HELD] {
            let mut speaker = FakeSpeaker::default();
            let mut sound = Sound::quiet();
            sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
            sound.tick(quiet, LOUDEST, &mut speaker)?;
            sound.tick(quiet, LOUDEST, &mut speaker)?;
            assert_eq!(speaker.calls, [START, Call::Stop]);
            assert!(!sound.playing());
        }
        Ok(())
    }

    #[test]
    fn a_dismissed_alarm_sounds_again_when_it_rings_again() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Quiet, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert_eq!(speaker.calls, [START, Call::Stop, START]);
        Ok(())
    }

    #[test]
    fn a_failing_speaker_is_left_alone_until_the_next_ring() {
        let mut speaker = FakeSpeaker::failing();
        let mut sound = Sound::quiet();
        assert!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker).is_err());
        assert!(!sound.playing());
        assert_eq!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker), Ok(()));
        assert_eq!(sound.tick(Ring::Quiet, LOUDEST, &mut speaker), Ok(()));
        assert!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker).is_err());
        assert_eq!(speaker.calls, [START, START]);
    }

    #[test]
    fn a_failed_stop_still_leaves_the_sound_quiet() {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        assert_eq!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker), Ok(()));
        speaker.failing = true;
        assert!(sound.tick(Ring::Quiet, LOUDEST, &mut speaker).is_err());
        assert!(!sound.playing());
        assert_eq!(sound.tick(Ring::Quiet, LOUDEST, &mut speaker), Ok(()));
        assert_eq!(speaker.calls, [START, Call::Stop]);
    }

    #[test]
    fn a_new_volume_reaches_the_speaker_while_it_rings_and_not_while_it_is_quiet()
    -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        let quieter = LOUDEST.down();
        sound.tick(Ring::Quiet, quieter, &mut speaker)?;
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Ringing, quieter, &mut speaker)?;
        sound.tick(Ring::Ringing, quieter, &mut speaker)?;
        sound.tick(Ring::Quiet, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Ringing, quieter, &mut speaker)?;
        assert_eq!(
            speaker.calls,
            [
                START,
                Call::Volume(quieter),
                Call::Stop,
                Call::Start(quieter)
            ]
        );
        Ok(())
    }

    #[test]
    fn the_volume_steps_between_one_and_fifteen_and_stops_at_either_end() {
        assert_eq!(Volume::new(0), None);
        assert_eq!(Volume::new(16), None);
        assert_eq!(Volume::new(1), Some(Volume::QUIETEST));
        assert_eq!(Volume::new(15), Some(LOUDEST));
        assert_eq!(LOUDEST.up(), LOUDEST);
        assert_eq!(Volume::QUIETEST.down(), Volume::QUIETEST);
        assert_eq!(LOUDEST.down().level(), 14);
        assert_eq!(Volume::QUIETEST.up().level(), 2);
    }

    #[test]
    fn the_loudest_volume_is_unity_and_each_step_down_is_three_decibels_quieter() {
        assert_eq!(LOUDEST.gain(), UNITY_GAIN);
        let mut volume = LOUDEST;
        while volume != Volume::QUIETEST {
            let quieter = volume.down();
            let decibels = 20.0 * (f64::from(quieter.gain()) / f64::from(volume.gain())).log10();
            assert!((decibels + 3.0).abs() < 0.01, "{decibels}");
            volume = quieter;
        }
        let whole = 20.0 * (f64::from(Volume::QUIETEST.gain()) / f64::from(UNITY_GAIN)).log10();
        assert!((whole + 42.0).abs() < 0.1, "{whole}");
    }
}
