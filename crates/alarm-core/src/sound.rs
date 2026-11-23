use crate::hardware::{HardwareError, Speaker};
use crate::ringer::Ring;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum State {
    #[default]
    Quiet,
    Playing,
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
        self.state == State::Playing
    }

    pub fn tick(&mut self, ring: Ring, speaker: &mut impl Speaker) -> Result<(), HardwareError> {
        match (ring, self.state) {
            (Ring::Ringing, State::Playing | State::Failed)
            | (Ring::Quiet | Ring::Snoozed(_), State::Quiet) => Ok(()),
            (Ring::Ringing, State::Quiet) => match speaker.start() {
                Ok(()) => {
                    self.state = State::Playing;
                    Ok(())
                }
                Err(error) => {
                    self.state = State::Failed;
                    Err(error)
                }
            },
            (Ring::Quiet | Ring::Snoozed(_), State::Playing) => {
                self.state = State::Quiet;
                speaker.stop()
            }
            (Ring::Quiet | Ring::Snoozed(_), State::Failed) => {
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

    const SNOOZED: Ring = Ring::Snoozed(UnixSeconds(1_794_512_818));

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Start,
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
        fn start(&mut self) -> Result<(), HardwareError> {
            self.calls.push(Call::Start);
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
        sound.tick(Ring::Quiet, &mut speaker)?;
        sound.tick(SNOOZED, &mut speaker)?;
        assert_eq!(speaker.calls, []);
        assert!(!sound.playing());
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_starts_the_sound_once() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        sound.tick(Ring::Ringing, &mut speaker)?;
        assert!(sound.playing());
        sound.tick(Ring::Ringing, &mut speaker)?;
        sound.tick(Ring::Ringing, &mut speaker)?;
        assert_eq!(speaker.calls, [Call::Start]);
        Ok(())
    }

    #[test]
    fn a_snooze_or_a_dismissal_stops_the_sound_at_once() -> Result<(), HardwareError> {
        for quiet in [SNOOZED, Ring::Quiet] {
            let mut speaker = FakeSpeaker::default();
            let mut sound = Sound::quiet();
            sound.tick(Ring::Ringing, &mut speaker)?;
            sound.tick(quiet, &mut speaker)?;
            sound.tick(quiet, &mut speaker)?;
            assert_eq!(speaker.calls, [Call::Start, Call::Stop]);
            assert!(!sound.playing());
        }
        Ok(())
    }

    #[test]
    fn a_snoozed_alarm_sounds_again_when_it_rings_again() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        sound.tick(Ring::Ringing, &mut speaker)?;
        sound.tick(SNOOZED, &mut speaker)?;
        sound.tick(Ring::Ringing, &mut speaker)?;
        assert_eq!(speaker.calls, [Call::Start, Call::Stop, Call::Start]);
        Ok(())
    }

    #[test]
    fn a_failing_speaker_is_left_alone_until_the_next_ring() {
        let mut speaker = FakeSpeaker::failing();
        let mut sound = Sound::quiet();
        assert!(sound.tick(Ring::Ringing, &mut speaker).is_err());
        assert!(!sound.playing());
        assert_eq!(sound.tick(Ring::Ringing, &mut speaker), Ok(()));
        assert_eq!(sound.tick(Ring::Quiet, &mut speaker), Ok(()));
        assert!(sound.tick(Ring::Ringing, &mut speaker).is_err());
        assert_eq!(speaker.calls, [Call::Start, Call::Start]);
    }

    #[test]
    fn a_failed_stop_still_leaves_the_sound_quiet() {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        assert_eq!(sound.tick(Ring::Ringing, &mut speaker), Ok(()));
        speaker.failing = true;
        assert!(sound.tick(Ring::Quiet, &mut speaker).is_err());
        assert!(!sound.playing());
        assert_eq!(sound.tick(Ring::Quiet, &mut speaker), Ok(()));
        assert_eq!(speaker.calls, [Call::Start, Call::Stop]);
    }
}
