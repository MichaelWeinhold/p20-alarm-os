use std::io;
use std::path::Path;
use std::time::Duration;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const HIFI_MISC: &str = "/dev/hifi_misc";
pub const REGISTERS: usize = 34;
pub const LARGEST_BODY: u32 = 4 << 20;

const SYS_IOCTL: u64 = 29;
const ENTRY: u64 = 0;
const SIGSTOP: i32 = 19;
const SIGTRAP: i32 = 5;
const SYSCALL_TRAP: i32 = SIGTRAP | 0x80;
const STOPPED: i32 = 0x7f;
const HIFI_TYPE: u32 = b'A' as u32;
const HIFI_FIRST: u32 = 0x70;
const ASYNC_ARGUMENT: usize = 12;
const SYNC_ARGUMENT: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub program: String,
    pub recording: String,
    pub count: usize,
    pub within: Duration,
}

pub fn options(arguments: &[String]) -> Result<Options, String> {
    let [_, program, recording, count, seconds] = arguments else {
        return Err("usage: dsp-recorder <program> <recording> <count> <seconds>".to_owned());
    };
    let count = count
        .parse()
        .map_err(|error| format!("count {count:?}: {error}"))?;
    let seconds = seconds
        .parse()
        .map_err(|error| format!("seconds {seconds:?}: {error}"))?;
    Ok(Options {
        program: program.clone(),
        recording: recording.clone(),
        count,
        within: Duration::from_secs(seconds),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Gone,
    Syscall,
    Quiet,
    Signal(i32),
}

#[must_use]
pub fn stop(status: i32) -> Stop {
    if status & 0xff != STOPPED {
        return Stop::Gone;
    }
    let signal = (status >> 8) & 0xff;
    if status >> 16 != 0 || signal == SIGSTOP || signal == SIGTRAP {
        Stop::Quiet
    } else if signal == SYSCALL_TRAP {
        Stop::Syscall
    } else {
        Stop::Signal(signal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ioctl {
    pub descriptor: i32,
    pub request: u32,
    pub argument: u64,
}

#[must_use]
pub fn ioctl_entered(registers: &[u64; REGISTERS]) -> Option<Ioctl> {
    if registers[7] != ENTRY || registers[8] != SYS_IOCTL {
        return None;
    }
    Some(Ioctl {
        descriptor: i32::try_from(registers[0]).ok()?,
        request: u32::try_from(registers[1] & 0xffff_ffff).ok()?,
        argument: registers[2],
    })
}

#[must_use]
pub fn hifi_request(request: u32) -> bool {
    (request >> 8) & 0xff == HIFI_TYPE && request & 0xff >= HIFI_FIRST
}

#[must_use]
pub fn argument_size(request: u32) -> usize {
    ((request >> 16) & 0x3fff) as usize
}

#[must_use]
pub fn hifi_descriptor(target: &Path) -> bool {
    target == Path::new(HIFI_MISC)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argument {
    pub body: u64,
    pub length: u32,
    pub reply: u32,
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

pub fn argument(bytes: &[u8]) -> io::Result<Argument> {
    let invalid = |reason: String| io::Error::new(io::ErrorKind::InvalidData, reason);
    if bytes.len() != ASYNC_ARGUMENT && bytes.len() != SYNC_ARGUMENT {
        return Err(invalid(format!("an argument of {} bytes", bytes.len())));
    }
    let short = || invalid("a short argument".to_owned());
    let body = u64::from(word(bytes, 0).ok_or_else(short)?)
        | (u64::from(word(bytes, 4).ok_or_else(short)?) << 32);
    let length = word(bytes, 8).ok_or_else(short)?;
    if length > LARGEST_BODY {
        return Err(invalid(format!("a body of {length} bytes")));
    }
    let reply = if bytes.len() == SYNC_ARGUMENT {
        word(bytes, 20).ok_or_else(short)?
    } else {
        0
    };
    Ok(Argument {
        body,
        length,
        reply,
    })
}

pub fn entry(request: u32, reply: u32, body: &[u8]) -> io::Result<Vec<u8>> {
    let length = u32::try_from(body.len())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut entry = Vec::with_capacity(12 + body.len());
    entry.extend_from_slice(&request.to_le_bytes());
    entry.extend_from_slice(&length.to_le_bytes());
    entry.extend_from_slice(&reply.to_le_bytes());
    entry.extend_from_slice(body);
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASYNC: u32 = 0xc00c_4170;
    const SYNC: u32 = 0x4018_4171;
    const WRITE_PARAMS: u32 = 0xc018_4175;
    const USB: u32 = 0x4018_417c;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn registers(entry: u64, number: u64, request: u64) -> [u64; REGISTERS] {
        let mut registers = [0; REGISTERS];
        registers[0] = 7;
        registers[1] = request;
        registers[2] = 0x71_9b51_e080;
        registers[7] = entry;
        registers[8] = number;
        registers
    }

    fn argument_bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    #[test]
    fn the_options_are_the_program_the_recording_a_count_and_seconds() -> Result<(), String> {
        let found = options(&arguments(&[
            "dsp-recorder",
            "/vendor/hal",
            "/data/rec",
            "12",
            "30",
        ]))?;
        assert_eq!(
            found,
            Options {
                program: "/vendor/hal".to_owned(),
                recording: "/data/rec".to_owned(),
                count: 12,
                within: Duration::from_secs(30),
            }
        );
        Ok(())
    }

    #[test]
    fn options_missing_or_not_numbers_are_refused() {
        assert!(options(&arguments(&["dsp-recorder", "/vendor/hal"])).is_err());
        assert!(
            options(&arguments(&[
                "dsp-recorder",
                "/hal",
                "/rec",
                "twelve",
                "30"
            ]))
            .is_err()
        );
        assert!(options(&arguments(&["dsp-recorder", "/hal", "/rec", "12", "-1"])).is_err());
    }

    #[test]
    fn a_syscall_stop_carries_the_trap_with_the_high_bit() {
        assert_eq!(stop(((SIGTRAP | 0x80) << 8) | STOPPED), Stop::Syscall);
    }

    #[test]
    fn an_event_a_stop_or_a_plain_trap_resumes_with_no_signal() {
        assert_eq!(stop((0x80 << 16) | (SIGTRAP << 8) | STOPPED), Stop::Quiet);
        assert_eq!(stop((3 << 16) | (SIGTRAP << 8) | STOPPED), Stop::Quiet);
        assert_eq!(stop((SIGSTOP << 8) | STOPPED), Stop::Quiet);
        assert_eq!(stop((SIGTRAP << 8) | STOPPED), Stop::Quiet);
    }

    #[test]
    fn another_signal_is_delivered() {
        assert_eq!(stop((17 << 8) | STOPPED), Stop::Signal(17));
    }

    #[test]
    fn an_exit_or_a_kill_is_gone() {
        assert_eq!(stop(0), Stop::Gone);
        assert_eq!(stop(1 << 8), Stop::Gone);
        assert_eq!(stop(9), Stop::Gone);
    }

    #[test]
    fn an_ioctl_is_read_at_its_entry_only() {
        let at_entry = registers(0, 29, u64::from(ASYNC));
        assert_eq!(
            ioctl_entered(&at_entry),
            Some(Ioctl {
                descriptor: 7,
                request: ASYNC,
                argument: 0x71_9b51_e080,
            })
        );
        assert_eq!(ioctl_entered(&registers(1, 29, u64::from(ASYNC))), None);
        assert_eq!(ioctl_entered(&registers(0, 64, u64::from(ASYNC))), None);
    }

    #[test]
    fn the_request_is_the_low_word_as_the_kernel_takes_it() {
        let extended = 0xffff_ffff_0000_0000 | u64::from(WRITE_PARAMS);
        let found = ioctl_entered(&registers(0, 29, extended));
        assert_eq!(found.map(|ioctl| ioctl.request), Some(WRITE_PARAMS));
    }

    #[test]
    fn the_hifi_requests_are_type_a_from_0x70() {
        for request in [ASYNC, SYNC, WRITE_PARAMS, USB] {
            assert!(hifi_request(request), "{request:08x}");
        }
        assert!(!hifi_request(0xc000_4150));
        assert!(!hifi_request(0xc110_5511));
    }

    #[test]
    fn the_argument_size_is_in_the_request() {
        assert_eq!(argument_size(ASYNC), 12);
        assert_eq!(argument_size(SYNC), 24);
        assert_eq!(argument_size(WRITE_PARAMS), 24);
    }

    #[test]
    fn only_hifi_misc_is_followed() {
        assert!(hifi_descriptor(Path::new("/dev/hifi_misc")));
        assert!(!hifi_descriptor(Path::new("/dev/snd/controlC0")));
    }

    #[test]
    fn an_async_argument_has_no_reply() -> io::Result<()> {
        let found = argument(&argument_bytes(&[0x9ae8_bb08, 0x71, 8]))?;
        assert_eq!(
            found,
            Argument {
                body: 0x71_9ae8_bb08,
                length: 8,
                reply: 0,
            }
        );
        Ok(())
    }

    #[test]
    fn a_sync_argument_carries_the_reply_length() -> io::Result<()> {
        let found = argument(&argument_bytes(&[0x9b51_e080, 0x71, 309_608, 0, 0x71, 4]))?;
        assert_eq!(found.length, 309_608);
        assert_eq!(found.reply, 4);
        Ok(())
    }

    #[test]
    fn an_argument_of_another_size_or_a_huge_body_is_refused() {
        assert!(argument(&argument_bytes(&[0, 0])).is_err());
        assert!(argument(&argument_bytes(&[0, 0, LARGEST_BODY + 1])).is_err());
    }

    #[test]
    fn an_entry_is_the_request_the_lengths_and_the_body() -> io::Result<()> {
        let found = entry(SYNC, 8, &[0x53, 0xdd, 0, 0])?;
        let mut wanted = argument_bytes(&[SYNC, 4, 8]);
        wanted.extend_from_slice(&[0x53, 0xdd, 0, 0]);
        assert_eq!(found, wanted);
        Ok(())
    }
}
