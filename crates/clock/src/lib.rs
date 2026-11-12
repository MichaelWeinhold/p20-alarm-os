use std::time::{SystemTime, UNIX_EPOCH};

use alarm_core::hardware::{Clock, HardwareError};
use alarm_core::time::UnixSeconds;

const DEVICE: &str = "realtime clock";

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

fn since_epoch(now: SystemTime) -> Result<UnixSeconds, HardwareError> {
    let elapsed = now
        .duration_since(UNIX_EPOCH)
        .map_err(|error| HardwareError::Failed {
            device: DEVICE,
            reason: error.to_string(),
        })?;
    i64::try_from(elapsed.as_secs())
        .map(UnixSeconds)
        .map_err(|error| HardwareError::Failed {
            device: DEVICE,
            reason: error.to_string(),
        })
}

impl Clock for SystemClock {
    fn now(&self) -> Result<UnixSeconds, HardwareError> {
        since_epoch(SystemTime::now())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_time_after_the_epoch_is_its_whole_seconds() -> Result<(), HardwareError> {
        let now = UNIX_EPOCH + Duration::from_millis(1_794_512_278_900);
        assert_eq!(since_epoch(now)?, UnixSeconds(1_794_512_278));
        Ok(())
    }

    #[test]
    fn a_time_before_the_epoch_is_a_failure_not_a_zero() {
        let before = UNIX_EPOCH - Duration::from_secs(1);
        assert!(matches!(
            since_epoch(before),
            Err(HardwareError::Failed { device: DEVICE, .. })
        ));
    }
}
