use std::ffi::{CStr, CString, c_ulong};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, symlink};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use alarm_core::hardware::{HardwareError, Modem};
use pid1::linux::{MS_NOATIME, MS_NODEV, MS_NOSUID, MS_RDONLY, mounted_from, named, named_devices};

use crate::{
    Access, BOOT, BY_NAME, MOUNTS, Mount, PATH, RADIO, RADIO_OFF, Reply, SCRATCH, STATE, SYSBOOT,
    TEE_DIRECTORIES, TEECD, VCOM, radio_off, ready, reply,
};

const O_NOCTTY: i32 = 0o400;
const TEECD_SETTLE: Duration = Duration::from_secs(1);
const READY_WITHIN: Duration = Duration::from_secs(30);
const READY_POLL: Duration = Duration::from_millis(250);
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
const RADIO_WITHIN: Duration = Duration::from_secs(60);
const RADIO_POLL: Duration = Duration::from_millis(500);
const READ_BYTES: usize = 512;

fn failed(device: &'static str, error: &io::Error) -> HardwareError {
    if error.kind() == io::ErrorKind::NotFound {
        HardwareError::Absent(device)
    } else {
        HardwareError::Failed {
            device,
            reason: error.to_string(),
        }
    }
}

fn text(device: &'static str, value: &str) -> Result<CString, HardwareError> {
    CString::new(value).map_err(|error| HardwareError::Failed {
        device,
        reason: error.to_string(),
    })
}

fn mounted(
    device: &'static str,
    source: &str,
    target: &str,
    filesystem: &CStr,
    flags: c_ulong,
    data: Option<&str>,
) -> Result<(), HardwareError> {
    fs::create_dir_all(target).map_err(|error| failed(device, &error))?;
    let data = data.map(|data| text(device, data)).transpose()?;
    mounted_from(
        &text(device, source)?,
        &text(device, target)?,
        filesystem,
        flags,
        data.as_deref(),
    )
    .map_err(|error| failed(device, &error))
}

fn partition_mounted(mount: Mount) -> Result<(), HardwareError> {
    let device = named(mount.partition).ok_or(HardwareError::Absent(mount.partition))?;
    let flags = match mount.access {
        Access::ReadOnly => MS_RDONLY,
        Access::Modem | Access::SecureStorage => MS_NOSUID | MS_NODEV | MS_NOATIME,
    };
    mounted(
        mount.partition,
        &format!("/dev/{device}"),
        mount.target,
        c"ext4",
        flags,
        mount.access.options(),
    )
}

fn linked() -> Result<(), HardwareError> {
    fs::create_dir_all(BY_NAME).map_err(|error| failed("by-name", &error))?;
    for (name, device) in named_devices().map_err(|error| failed("by-name", &error))? {
        match symlink(format!("/dev/{device}"), format!("{BY_NAME}/{name}")) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
                return Err(failed("by-name", &error));
            }
            _ => {}
        }
    }
    Ok(())
}

fn prepared() -> Result<(), HardwareError> {
    linked()?;
    for target in SCRATCH {
        mounted("tmpfs", "tmpfs", target, c"tmpfs", 0, None)?;
    }
    for directory in TEE_DIRECTORIES {
        fs::create_dir_all(directory).map_err(|error| failed("tmpfs", &error))?;
    }
    MOUNTS.into_iter().try_for_each(partition_mounted)
}

fn teecd() -> Result<Child, HardwareError> {
    let log = pid1::linux::kmsg().map_err(|error| failed("kmsg", &error))?;
    let mut child = Command::new(TEECD)
        .env_clear()
        .env("PATH", PATH)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().map_err(|error| failed("kmsg", &error))?,
        ))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|error| failed("teecd", &error))?;
    thread::sleep(TEECD_SETTLE);
    match child.try_wait() {
        Ok(None) => Ok(child),
        Ok(Some(status)) => Err(HardwareError::Failed {
            device: "teecd",
            reason: format!("exited with {status}"),
        }),
        Err(error) => Err(failed("teecd", &error)),
    }
}

fn sysbooted() -> Result<(), HardwareError> {
    fs::write(SYSBOOT, BOOT).map_err(|error| failed("his_modem", &error))?;
    let deadline = Instant::now() + READY_WITHIN;
    loop {
        let bytes = fs::read(STATE).map_err(|error| failed("balong_power", &error))?;
        if ready(&bytes)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(HardwareError::Failed {
                device: "balong_power",
                reason: format!(
                    "not ready after {} s: {:?}",
                    READY_WITHIN.as_secs(),
                    String::from_utf8_lossy(&bytes)
                ),
            });
        }
        thread::sleep(READY_POLL);
    }
}

type Answers = Receiver<io::Result<Vec<u8>>>;

fn answers(vcom: &File) -> Result<Answers, HardwareError> {
    let mut vcom = vcom
        .try_clone()
        .map_err(|error| failed("appvcom", &error))?;
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("appvcom".to_owned())
        .spawn(move || {
            let mut buffer = [0; READ_BYTES];
            loop {
                let read = match vcom.read(&mut buffer) {
                    Ok(0) => Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                    read => read.map(|count| buffer[..count].to_vec()),
                };
                let ended = read.is_err();
                if sender.send(read).is_err() || ended {
                    return;
                }
            }
        })
        .map_err(|error| failed("appvcom", &error))?;
    Ok(receiver)
}

fn answered(
    vcom: &mut File,
    answers: &Answers,
    command: &str,
) -> Result<Vec<String>, HardwareError> {
    vcom.write_all(command.as_bytes())
        .map_err(|error| failed("appvcom", &error))?;
    let deadline = Instant::now() + ANSWER_WITHIN;
    let mut read = Vec::new();
    loop {
        match reply(&read) {
            Reply::Answered(lines) => return Ok(lines),
            Reply::Refused(line) => {
                return Err(HardwareError::Failed {
                    device: "appvcom",
                    reason: format!("{:?} answered {line:?}", command.trim_end()),
                });
            }
            Reply::Pending => {}
        }
        match answers.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(bytes)) => read.extend_from_slice(&bytes),
            Ok(Err(error)) => return Err(failed("appvcom", &error)),
            Err(_) => {
                return Err(HardwareError::Failed {
                    device: "appvcom",
                    reason: format!(
                        "no answer to {:?} in {} s: {:?}",
                        command.trim_end(),
                        ANSWER_WITHIN.as_secs(),
                        String::from_utf8_lossy(&read)
                    ),
                });
            }
        }
    }
}

fn radio_turned_off(vcom: &mut File, answers: &Answers) -> Result<(), HardwareError> {
    answered(vcom, answers, RADIO_OFF)?;
    radio_off(&answered(vcom, answers, RADIO)?)
}

#[derive(Debug)]
pub struct Balong {
    teecd: Option<Child>,
}

impl Balong {
    pub fn open() -> Result<Self, HardwareError> {
        if Path::new(SYSBOOT).is_file() {
            Ok(Self { teecd: None })
        } else {
            Err(HardwareError::Absent("his_modem"))
        }
    }
}

impl Modem for Balong {
    fn boot(&mut self) -> Result<(), HardwareError> {
        prepared()?;
        if self.teecd.is_none() {
            self.teecd = Some(teecd()?);
        }
        sysbooted()
    }

    fn radio_off(&mut self) -> Result<(), HardwareError> {
        let mut vcom = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(O_NOCTTY)
            .open(VCOM)
            .map_err(|error| failed("appvcom", &error))?;
        let answers = answers(&vcom)?;
        let deadline = Instant::now() + RADIO_WITHIN;
        loop {
            match radio_turned_off(&mut vcom, &answers) {
                Err(_) if Instant::now() < deadline => thread::sleep(RADIO_POLL),
                turned => return turned,
            }
        }
    }
}
