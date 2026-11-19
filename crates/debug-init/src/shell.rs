use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use pid1::kmsg;

pub const PROGRAM: &str = "/system/bin/sh";
pub const PATH: &str = "/system/bin";
pub const PAUSE: Duration = Duration::from_secs(1);
pub const RECOVERY: &str = "/mnt/recovery";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Restart,
    Recovery,
}

#[must_use]
pub fn next(marker: &Path) -> Next {
    if marker.exists() {
        Next::Recovery
    } else {
        Next::Restart
    }
}

#[must_use]
pub fn ended(status: ExitStatus, next: Next) -> String {
    let then = match next {
        Next::Restart => "starting it again".to_owned(),
        Next::Recovery => format!("{RECOVERY} asks for recovery, rebooting into it"),
    };
    kmsg::line(&format!("debug: {PROGRAM} ended with {status}, {then}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::fs;
    use std::io;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn without_the_marker_the_shell_is_started_again() -> io::Result<()> {
        let scratch = Scratch::new("no-marker")?;
        assert_eq!(next(&scratch.0.join("recovery")), Next::Restart);
        Ok(())
    }

    #[test]
    fn the_marker_reboots_into_recovery() -> io::Result<()> {
        let scratch = Scratch::new("marker")?;
        let marker = scratch.0.join("recovery");
        fs::write(&marker, "")?;
        assert_eq!(next(&marker), Next::Recovery);
        Ok(())
    }

    #[test]
    fn the_log_says_how_the_shell_ended_and_what_comes_next() {
        assert_eq!(
            ended(ExitStatus::from_raw(0), Next::Restart),
            "<3>p20-alarm-os: debug: /system/bin/sh ended with exit status: 0, starting it again\n"
        );
        assert_eq!(
            ended(ExitStatus::from_raw(129 << 8), Next::Restart),
            "<3>p20-alarm-os: debug: /system/bin/sh ended with exit status: 129, starting it again\n"
        );
        assert!(ended(ExitStatus::from_raw(9), Next::Restart).ends_with(", starting it again\n"));
        assert_eq!(
            ended(ExitStatus::from_raw(0), Next::Recovery),
            "<3>p20-alarm-os: debug: /system/bin/sh ended with exit status: 0, /mnt/recovery asks for recovery, rebooting into it\n"
        );
    }
}
