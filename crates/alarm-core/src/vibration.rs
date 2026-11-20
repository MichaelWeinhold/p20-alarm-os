use crate::hardware::{HardwareError, Vibrator};
use crate::ringer::Ring;
use crate::time::UnixSeconds;

pub const PULSE_MILLISECONDS: u32 = 1_000;
pub const PERIOD_SECONDS: i64 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum State {
    #[default]
    Still,
    Pulsing(UnixSeconds),
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vibration {
    state: State,
}

impl Vibration {
    #[must_use]
    pub fn still() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn pulsing(&self) -> bool {
        matches!(self.state, State::Pulsing(_))
    }

    #[must_use]
    pub fn seconds_left(&self, now: UnixSeconds) -> Option<u64> {
        match self.state {
            State::Pulsing(next) => Some(next.0.saturating_sub(now.0).max(1).unsigned_abs()),
            State::Still | State::Failed => None,
        }
    }

    pub fn tick(
        &mut self,
        now: UnixSeconds,
        ring: Ring,
        vibrator: &mut impl Vibrator,
    ) -> Result<(), HardwareError> {
        match (ring, self.state) {
            (Ring::Ringing, State::Pulsing(next)) if now < next => Ok(()),
            (Ring::Ringing, State::Failed) | (Ring::Quiet | Ring::Snoozed(_), State::Still) => {
                Ok(())
            }
            (Ring::Ringing, State::Still | State::Pulsing(_)) => {
                match vibrator.vibrate(PULSE_MILLISECONDS) {
                    Ok(()) => {
                        self.state =
                            State::Pulsing(UnixSeconds(now.0.saturating_add(PERIOD_SECONDS)));
                        Ok(())
                    }
                    Err(error) => {
                        self.state = State::Failed;
                        Err(error)
                    }
                }
            }
            (Ring::Quiet | Ring::Snoozed(_), State::Pulsing(_)) => {
                self.state = State::Still;
                vibrator.stop()
            }
            (Ring::Quiet | Ring::Snoozed(_), State::Failed) => {
                self.state = State::Still;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: UnixSeconds = UnixSeconds(1_794_512_278);
    const SNOOZED: Ring = Ring::Snoozed(UnixSeconds(1_794_512_818));

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Vibrate(u32),
        Stop,
    }

    #[derive(Default)]
    struct FakeVibrator {
        calls: Vec<Call>,
        failing: bool,
    }

    impl FakeVibrator {
        fn failing() -> Self {
            Self {
                calls: Vec::new(),
                failing: true,
            }
        }

        fn answer(&self) -> Result<(), HardwareError> {
            if self.failing {
                Err(HardwareError::Failed {
                    device: "vibrator",
                    reason: "refused".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    impl Vibrator for FakeVibrator {
        fn vibrate(&mut self, milliseconds: u32) -> Result<(), HardwareError> {
            self.calls.push(Call::Vibrate(milliseconds));
            self.answer()
        }

        fn stop(&mut self) -> Result<(), HardwareError> {
            self.calls.push(Call::Stop);
            self.answer()
        }
    }

    fn at(seconds: i64) -> UnixSeconds {
        UnixSeconds(START.0 + seconds)
    }

    const PULSE: Call = Call::Vibrate(PULSE_MILLISECONDS);

    #[test]
    fn a_quiet_alarm_never_touches_the_vibrator() -> Result<(), HardwareError> {
        let mut vibrator = FakeVibrator::default();
        let mut vibration = Vibration::still();
        vibration.tick(at(0), Ring::Quiet, &mut vibrator)?;
        vibration.tick(at(5), SNOOZED, &mut vibrator)?;
        assert_eq!(vibrator.calls, []);
        assert_eq!(vibration.seconds_left(at(5)), None);
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_pulses_once_a_period() -> Result<(), HardwareError> {
        let mut vibrator = FakeVibrator::default();
        let mut vibration = Vibration::still();
        vibration.tick(at(0), Ring::Ringing, &mut vibrator)?;
        assert!(vibration.pulsing());
        assert_eq!(vibration.seconds_left(at(0)), Some(2));
        vibration.tick(at(1), Ring::Ringing, &mut vibrator)?;
        assert_eq!(vibration.seconds_left(at(1)), Some(1));
        vibration.tick(at(2), Ring::Ringing, &mut vibrator)?;
        vibration.tick(at(5), Ring::Ringing, &mut vibrator)?;
        assert_eq!(vibrator.calls, [PULSE, PULSE, PULSE]);
        assert_eq!(vibration.seconds_left(at(9)), Some(1));
        Ok(())
    }

    #[test]
    fn a_snooze_or_a_dismissal_stops_the_vibrator_at_once() -> Result<(), HardwareError> {
        for quiet in [SNOOZED, Ring::Quiet] {
            let mut vibrator = FakeVibrator::default();
            let mut vibration = Vibration::still();
            vibration.tick(at(0), Ring::Ringing, &mut vibrator)?;
            vibration.tick(at(1), quiet, &mut vibrator)?;
            vibration.tick(at(3), quiet, &mut vibrator)?;
            assert_eq!(vibrator.calls, [PULSE, Call::Stop]);
            assert!(!vibration.pulsing());
            assert_eq!(vibration.seconds_left(at(3)), None);
        }
        Ok(())
    }

    #[test]
    fn a_snoozed_alarm_pulses_again_when_it_rings_again() -> Result<(), HardwareError> {
        let mut vibrator = FakeVibrator::default();
        let mut vibration = Vibration::still();
        vibration.tick(at(0), Ring::Ringing, &mut vibrator)?;
        vibration.tick(at(1), SNOOZED, &mut vibrator)?;
        vibration.tick(at(540), Ring::Ringing, &mut vibrator)?;
        assert_eq!(vibrator.calls, [PULSE, Call::Stop, PULSE]);
        Ok(())
    }

    #[test]
    fn a_failing_vibrator_is_left_alone_until_the_next_ring() {
        let mut vibrator = FakeVibrator::failing();
        let mut vibration = Vibration::still();
        assert!(vibration.tick(at(0), Ring::Ringing, &mut vibrator).is_err());
        assert_eq!(vibration.seconds_left(at(0)), None);
        assert_eq!(vibration.tick(at(2), Ring::Ringing, &mut vibrator), Ok(()));
        assert_eq!(vibration.tick(at(4), Ring::Quiet, &mut vibrator), Ok(()));
        assert!(vibration.tick(at(6), Ring::Ringing, &mut vibrator).is_err());
        assert_eq!(vibrator.calls, [PULSE, PULSE]);
    }
}
