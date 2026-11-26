use crate::hardware::{HardwareError, Speaker};
use crate::ringer::Ring;

pub const UNITY_GAIN: u32 = 1 << 16;
pub const LONGEST_NAME: usize = 64;
pub const NAME_SUFFIX: &str = ".wav";
const STEP_GAIN: u32 = 46_396;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoundName(String);

impl SoundName {
    #[must_use]
    pub fn new(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(NAME_SUFFIX)?;
        let named = name.len() <= LONGEST_NAME
            && stem.starts_with(|first: char| first.is_ascii_alphanumeric())
            && stem
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        named.then(|| Self(name.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn stem(&self) -> &str {
        self.0.strip_suffix(NAME_SUFFIX).unwrap_or(&self.0)
    }
}

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

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
enum Loaded {
    #[default]
    Unread,
    Ready(SoundName),
    Unready(Option<SoundName>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sound {
    state: State,
    loaded: Loaded,
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

    pub fn choose(
        &mut self,
        chosen: Option<&SoundName>,
        speaker: &mut impl Speaker,
    ) -> Option<Result<(), HardwareError>> {
        let held = match &self.loaded {
            Loaded::Unread => None,
            Loaded::Ready(sound) => Some(Some(sound)),
            Loaded::Unready(sound) => Some(sound.as_ref()),
        };
        if held == Some(chosen) {
            return None;
        }
        let Some(sound) = chosen else {
            self.loaded = Loaded::Unready(None);
            return Some(Ok(()));
        };
        let read = speaker.load(sound);
        self.loaded = match read {
            Ok(()) => Loaded::Ready(sound.clone()),
            Err(_) => Loaded::Unready(Some(sound.clone())),
        };
        Some(read)
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
            | (Ring::Quiet | Ring::Held(_) | Ring::Paused, State::Quiet) => Ok(()),
            (Ring::Ringing, State::Quiet) if !matches!(self.loaded, Loaded::Ready(_)) => Ok(()),
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
            (Ring::Quiet | Ring::Held(_) | Ring::Paused, State::Playing(_)) => {
                self.state = State::Quiet;
                speaker.stop()
            }
            (Ring::Quiet | Ring::Held(_) | Ring::Paused, State::Failed) => {
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
        Load(&'static str),
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
        fn load(&mut self, sound: &SoundName) -> Result<(), HardwareError> {
            let name = match sound.as_str() {
                "Carbon.wav" => "Carbon.wav",
                "Argon.wav" => "Argon.wav",
                _ => "other",
            };
            self.calls.push(Call::Load(name));
            self.answer()
        }

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

    fn named(name: &str) -> Result<SoundName, HardwareError> {
        SoundName::new(name).ok_or(HardwareError::Absent("sound name"))
    }

    fn ready(speaker: &mut FakeSpeaker) -> Result<Sound, HardwareError> {
        let mut sound = Sound::quiet();
        sound
            .choose(Some(&named("Carbon.wav")?), speaker)
            .transpose()?;
        speaker.calls.clear();
        Ok(sound)
    }

    #[test]
    fn the_chosen_sound_is_loaded_once_and_again_only_when_the_choice_changes()
    -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        let carbon = named("Carbon.wav")?;
        let argon = named("Argon.wav")?;
        assert_eq!(sound.choose(Some(&carbon), &mut speaker), Some(Ok(())));
        assert_eq!(sound.choose(Some(&carbon), &mut speaker), None);
        assert_eq!(sound.choose(Some(&argon), &mut speaker), Some(Ok(())));
        assert_eq!(sound.choose(Some(&argon), &mut speaker), None);
        assert_eq!(
            speaker.calls,
            [Call::Load("Carbon.wav"), Call::Load("Argon.wav")]
        );
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert!(sound.playing());
        Ok(())
    }

    #[test]
    fn no_sound_chosen_rings_without_the_speaker() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = Sound::quiet();
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert_eq!(sound.choose(None, &mut speaker), Some(Ok(())));
        assert_eq!(sound.choose(None, &mut speaker), None);
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Quiet, LOUDEST, &mut speaker)?;
        assert_eq!(speaker.calls, []);
        assert!(!sound.playing());
        Ok(())
    }

    #[test]
    fn a_sound_that_does_not_load_is_never_started_and_never_swapped_for_another()
    -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::failing();
        let mut sound = Sound::quiet();
        let carbon = named("Carbon.wav")?;
        assert!(matches!(
            sound.choose(Some(&carbon), &mut speaker),
            Some(Err(_))
        ));
        assert_eq!(sound.choose(Some(&carbon), &mut speaker), None);
        speaker.failing = false;
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert!(!sound.playing());
        assert_eq!(speaker.calls, [Call::Load("Carbon.wav")]);
        Ok(())
    }

    #[test]
    fn a_quiet_alarm_never_touches_the_speaker() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = ready(&mut speaker)?;
        sound.tick(Ring::Quiet, LOUDEST, &mut speaker)?;
        sound.tick(HELD, LOUDEST, &mut speaker)?;
        assert_eq!(speaker.calls, []);
        assert!(!sound.playing());
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_starts_the_sound_once() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = ready(&mut speaker)?;
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
            let mut sound = ready(&mut speaker)?;
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
        let mut sound = ready(&mut speaker)?;
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Quiet, LOUDEST, &mut speaker)?;
        sound.tick(Ring::Ringing, LOUDEST, &mut speaker)?;
        assert_eq!(speaker.calls, [START, Call::Stop, START]);
        Ok(())
    }

    #[test]
    fn a_failing_speaker_is_left_alone_until_the_next_ring() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = ready(&mut speaker)?;
        speaker.failing = true;
        assert!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker).is_err());
        assert!(!sound.playing());
        assert_eq!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker), Ok(()));
        assert_eq!(sound.tick(Ring::Quiet, LOUDEST, &mut speaker), Ok(()));
        assert!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker).is_err());
        assert_eq!(speaker.calls, [START, START]);
        Ok(())
    }

    #[test]
    fn a_failed_stop_still_leaves_the_sound_quiet() -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = ready(&mut speaker)?;
        assert_eq!(sound.tick(Ring::Ringing, LOUDEST, &mut speaker), Ok(()));
        speaker.failing = true;
        assert!(sound.tick(Ring::Quiet, LOUDEST, &mut speaker).is_err());
        assert!(!sound.playing());
        assert_eq!(sound.tick(Ring::Quiet, LOUDEST, &mut speaker), Ok(()));
        assert_eq!(speaker.calls, [START, Call::Stop]);
        Ok(())
    }

    #[test]
    fn a_new_volume_reaches_the_speaker_while_it_rings_and_not_while_it_is_quiet()
    -> Result<(), HardwareError> {
        let mut speaker = FakeSpeaker::default();
        let mut sound = ready(&mut speaker)?;
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
    fn a_sound_name_is_a_plain_wav_file_name_of_at_most_sixty_four_bytes() {
        let longest = format!("{}.wav", "a".repeat(LONGEST_NAME - 4));
        for good in [
            "ringtone.wav",
            "Carbon.wav",
            "a.wav",
            "0_x-y.z.wav",
            &longest,
        ] {
            assert_eq!(
                SoundName::new(good).map(|name| name.0),
                Some(good.to_owned())
            );
        }
        let longer = format!("a{longest}");
        for bad in [
            "",
            ".wav",
            "ringtone",
            "ringtone.WAV",
            "ringtone.ogg",
            ".hidden.wav",
            "-a.wav",
            "a b.wav",
            "a/b.wav",
            "../a.wav",
            "ä.wav",
            &longer,
        ] {
            assert_eq!(SoundName::new(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_sound_name_shows_without_its_suffix() {
        let name = SoundName::new("Carbon.wav");
        assert_eq!(name.as_ref().map(SoundName::stem), Some("Carbon"));
        assert_eq!(name.as_ref().map(SoundName::as_str), Some("Carbon.wav"));
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
