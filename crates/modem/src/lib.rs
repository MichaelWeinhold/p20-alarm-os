use alarm_core::hardware::HardwareError;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const SYSBOOT: &str = "/sys/devices/platform/his_modem/modem_sysboot_start";
pub const STATE: &str = "/sys/devices/platform/balong_power/state";
pub const VCOM: &str = "/dev/appvcom";
pub const TEECD: &str = "/vendor/bin/teecd";
pub const PATH: &str = "/system/bin";
pub const SCRATCH: [&str; 2] = ["/mnt", "/data"];
pub const TEE_DIRECTORIES: [&str; 2] = ["/data/sec_storage_data", "/data/sec_storage_data_users"];
pub const BOOT: &str = "1\n";
pub const RADIO_OFF: &str = "AT+CFUN=0\r";
pub const RADIO: &str = "AT+CFUN?\r";

const STATE_DEVICE: &str = "balong_power";
const VCOM_DEVICE: &str = "appvcom";
const READY: &str = "MODEM_STATE_READY";
const STATE_PREFIX: &str = "MODEM_STATE_";
const FINAL_OK: &str = "OK";
const REFUSALS: [&str; 5] = [
    "ERROR",
    "COMMAND NOT SUPPORT",
    "TOO MANY PARAMETERS",
    "+CME ERROR",
    "+CMS ERROR",
];
const FUNCTION: &str = "+CFUN:";
const MINIMUM_FUNCTION: &str = "0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    Modem,
    SecureStorage,
}

impl Access {
    #[must_use]
    pub fn options(self) -> Option<&'static str> {
        match self {
            Self::ReadOnly => None,
            Self::Modem => Some("noauto_da_alloc,data=ordered,barrier=1"),
            Self::SecureStorage => Some("discard,auto_da_alloc,data=journal"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mount {
    pub partition: &'static str,
    pub target: &'static str,
    pub access: Access,
}

pub const MOUNTS: [Mount; 6] = [
    Mount {
        partition: "vendor",
        target: "/vendor",
        access: Access::ReadOnly,
    },
    Mount {
        partition: "modem_secure",
        target: "/mnt/modem/modem_secure",
        access: Access::Modem,
    },
    Mount {
        partition: "modem_om",
        target: "/modem_log",
        access: Access::Modem,
    },
    Mount {
        partition: "modem_fw",
        target: "/vendor/modem/modem_fw",
        access: Access::ReadOnly,
    },
    Mount {
        partition: "modemnvm_img",
        target: "/mnt/modem/mnvm2:0",
        access: Access::Modem,
    },
    Mount {
        partition: "secure_storage",
        target: "/sec_storage",
        access: Access::SecureStorage,
    },
];

fn unexpected(device: &'static str, bytes: &[u8]) -> HardwareError {
    HardwareError::Failed {
        device,
        reason: format!("unexpected {:?}", String::from_utf8_lossy(bytes)),
    }
}

pub fn ready(bytes: &[u8]) -> Result<bool, HardwareError> {
    let state = std::str::from_utf8(bytes)
        .map(|text| text.trim_end_matches(['\0', '\n']))
        .map_err(|_| unexpected(STATE_DEVICE, bytes))?;
    match state.strip_prefix(STATE_PREFIX) {
        Some(_) if state == READY => Ok(true),
        Some(rest)
            if !rest.is_empty()
                && rest
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'_') =>
        {
            Ok(false)
        }
        _ => Err(unexpected(STATE_DEVICE, bytes)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Pending,
    Answered(Vec<String>),
    Refused(String),
}

#[must_use]
pub fn reply(bytes: &[u8]) -> Reply {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    for line in text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line == FINAL_OK {
            return Reply::Answered(lines);
        }
        if REFUSALS.iter().any(|refusal| line.starts_with(refusal)) {
            return Reply::Refused(line.to_owned());
        }
        lines.push(line.to_owned());
    }
    Reply::Pending
}

pub fn radio_off(lines: &[String]) -> Result<(), HardwareError> {
    match lines
        .iter()
        .find_map(|line| line.strip_prefix(FUNCTION))
        .map(str::trim)
    {
        Some(MINIMUM_FUNCTION) => Ok(()),
        Some(level) => Err(HardwareError::Failed {
            device: VCOM_DEVICE,
            reason: format!("the radio reads {FUNCTION} {level}"),
        }),
        None => Err(HardwareError::Failed {
            device: VCOM_DEVICE,
            reason: format!("no {FUNCTION} in {lines:?}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn a_booted_modem_reads_ready_past_its_newline_and_nul() {
        assert_eq!(ready(include_bytes!("../testdata/state-ready")), Ok(true));
    }

    #[test]
    fn any_other_state_is_not_ready_yet() {
        assert_eq!(ready(b"MODEM_STATE_OFF\n\0"), Ok(false));
        assert_eq!(ready(b"MODEM_STATE_INVALID\n"), Ok(false));
    }

    #[test]
    fn a_state_that_is_not_a_modem_state_is_a_failure() {
        for bytes in [&b""[..], b"\n\0", b"MODEM_STATE_\n", b"ready\n", b"\xff\n"] {
            assert!(
                matches!(
                    ready(bytes),
                    Err(HardwareError::Failed {
                        device: STATE_DEVICE,
                        ..
                    })
                ),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn the_radio_off_command_is_answered_past_its_echo() {
        assert_eq!(
            reply(include_bytes!("../testdata/cfun-set")),
            Reply::Answered(lines(&["AT+CFUN=0"]))
        );
    }

    #[test]
    fn the_radio_reads_off_at_the_minimum_function() -> Result<(), HardwareError> {
        let Reply::Answered(read) = reply(include_bytes!("../testdata/cfun-read")) else {
            return Err(unexpected(VCOM_DEVICE, b"cfun-read"));
        };
        assert_eq!(read, lines(&["AT+CFUN?", "+CFUN: 0"]));
        radio_off(&read)
    }

    #[test]
    fn an_unsupported_command_is_refused_rather_than_waited_on() {
        assert_eq!(
            reply(include_bytes!("../testdata/not-supported")),
            Reply::Refused("COMMAND NOT SUPPORT".to_owned())
        );
        assert_eq!(
            reply(b"AT+CFUN=0\r\r\n+CME ERROR: 3\r\n"),
            Reply::Refused("+CME ERROR: 3".to_owned())
        );
    }

    #[test]
    fn the_queued_reports_before_an_answer_are_passed_over() {
        let read = b"\r\n^SIMST:255,0\r\n\r\n^SRVST: 0\r\n\r\n^SRCHEDPLMNINFO:0,1,26203,51157\r\nAT+CFUN=0\r\r\nOK\r\n";
        assert_eq!(
            reply(read),
            Reply::Answered(lines(&[
                "^SIMST:255,0",
                "^SRVST: 0",
                "^SRCHEDPLMNINFO:0,1,26203,51157",
                "AT+CFUN=0",
            ]))
        );
    }

    #[test]
    fn a_reply_without_its_final_line_is_still_pending() {
        let whole = include_bytes!("../testdata/cfun-read");
        assert_eq!(reply(&whole[..whole.len() - 4]), Reply::Pending);
        assert_eq!(reply(b""), Reply::Pending);
    }

    #[test]
    fn a_radio_at_any_other_function_is_not_off() {
        assert!(radio_off(&lines(&["+CFUN: 1"])).is_err());
        assert!(radio_off(&lines(&["+CFUN: 4"])).is_err());
        assert!(radio_off(&lines(&["AT+CFUN?"])).is_err());
    }

    #[test]
    fn a_mount_inside_another_comes_after_it() {
        for (index, mount) in MOUNTS.iter().enumerate() {
            for (other, outer) in MOUNTS.iter().enumerate() {
                if mount.target.starts_with(&format!("{}/", outer.target)) {
                    assert!(other < index, "{} before {}", outer.target, mount.target);
                }
            }
        }
    }
}
