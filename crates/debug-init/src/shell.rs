use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::time::Duration;

use pid1::kmsg;

pub const PROGRAM: &str = "/system/bin/sh";
pub const PATH: &str = "/system/bin";
pub const PAUSE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Restart,
    Recovery,
}

#[must_use]
pub fn next(status: ExitStatus) -> Next {
    if status.signal().is_some() {
        Next::Restart
    } else {
        Next::Recovery
    }
}

#[must_use]
pub fn ended(status: ExitStatus) -> String {
    let then = match next(status) {
        Next::Restart => "starting it again",
        Next::Recovery => "rebooting into recovery",
    };
    kmsg::line(&format!("debug: {PROGRAM} ended with {status}, {then}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_reboots_into_recovery_whatever_its_status() {
        assert_eq!(next(ExitStatus::from_raw(0)), Next::Recovery);
        assert_eq!(next(ExitStatus::from_raw(1 << 8)), Next::Recovery);
        assert_eq!(next(ExitStatus::from_raw(130 << 8)), Next::Recovery);
    }

    #[test]
    fn a_shell_killed_by_a_signal_is_started_again() {
        assert_eq!(next(ExitStatus::from_raw(9)), Next::Restart);
        assert_eq!(next(ExitStatus::from_raw(1)), Next::Restart);
    }

    #[test]
    fn the_log_says_how_the_shell_ended_and_what_comes_next() {
        assert_eq!(
            ended(ExitStatus::from_raw(0)),
            "<3>p20-alarm-os: debug: /system/bin/sh ended with exit status: 0, rebooting into recovery\n"
        );
        assert!(ended(ExitStatus::from_raw(9)).ends_with(", starting it again\n"));
    }
}
