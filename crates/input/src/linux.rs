use std::collections::VecDeque;
use std::ffi::{CString, c_char, c_int, c_short, c_ulong, c_void};
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use alarm_core::hardware::{HardwareError, Input};
use alarm_core::input::InputEvent;

use crate::firmware::{BLOCKS, DIRECTORY, PARTITION, REQUESTS, partition, requested};
use crate::{DEVICES, Decoder, RECORD_SIZE, events};

const CLASS: &str = "/sys/class/input";
const NODES: &str = "/dev/input";
const MOUNT_POINT: &str = "/dev/odm";
const READ_RECORDS: usize = 64;
const RESCAN: Duration = Duration::from_secs(1);
const POLLIN: c_short = 0x1;
const POLLERR: c_short = 0x8;
const POLLHUP: c_short = 0x10;
const POLLNVAL: c_short = 0x20;
const MS_RDONLY: c_ulong = 0x1;
const EINTR: i32 = 4;
const EBUSY: i32 = 16;

#[repr(C)]
struct PollFd {
    descriptor: c_int,
    events: c_short,
    returned: c_short,
}

unsafe extern "C" {
    fn poll(descriptors: *mut PollFd, count: c_ulong, timeout: c_int) -> c_int;
    fn mount(
        source: *const c_char,
        target: *const c_char,
        filesystem: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
}

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

#[derive(Debug)]
struct Device {
    name: &'static str,
    file: Option<File>,
    decoder: Decoder,
}

impl Device {
    fn read(&mut self, pending: &mut VecDeque<InputEvent>) -> Result<(), HardwareError> {
        let Some(file) = &mut self.file else {
            return Ok(());
        };
        let mut buffer = [0; RECORD_SIZE * READ_RECORDS];
        let read = match file.read(&mut buffer) {
            Ok(0) => Err(failed(
                self.name,
                &io::Error::from(io::ErrorKind::UnexpectedEof),
            )),
            Ok(read) => events(self.name, &buffer[..read], &mut self.decoder),
            Err(error) => Err(failed(self.name, &error)),
        };
        match read {
            Ok(events) => {
                pending.extend(events);
                Ok(())
            }
            Err(error) => {
                self.file = None;
                self.decoder = Decoder::default();
                Err(error)
            }
        }
    }
}

#[derive(Debug)]
pub struct EvdevInput {
    devices: Vec<Device>,
    pending: VecDeque<InputEvent>,
}

impl EvdevInput {
    #[must_use]
    pub fn open() -> Self {
        let mut input = Self {
            devices: DEVICES
                .iter()
                .map(|&name| Device {
                    name,
                    file: None,
                    decoder: Decoder::default(),
                })
                .collect(),
            pending: VecDeque::new(),
        };
        input.scan();
        input
    }

    #[must_use]
    pub fn absent(&self) -> Vec<&'static str> {
        self.devices
            .iter()
            .filter(|device| device.file.is_none())
            .map(|device| device.name)
            .collect()
    }

    fn scan(&mut self) {
        if self.devices.iter().all(|device| device.file.is_some()) {
            return;
        }
        let Ok(entries) = fs::read_dir(CLASS) else {
            return;
        };
        for entry in entries.flatten() {
            let node = entry.file_name();
            let Some(node) = node.to_str().filter(|node| node.starts_with("event")) else {
                continue;
            };
            let Ok(name) = fs::read_to_string(entry.path().join("device/name")) else {
                continue;
            };
            if let Some(device) = self
                .devices
                .iter_mut()
                .find(|device| device.file.is_none() && device.name == name.trim())
            {
                device.file = File::open(Path::new(NODES).join(node)).ok();
            }
        }
    }

    fn poll(&mut self, timeout: Duration) -> Result<(), HardwareError> {
        let mut descriptors: Vec<PollFd> = self
            .devices
            .iter()
            .filter_map(|device| device.file.as_ref())
            .map(|file| PollFd {
                descriptor: file.as_raw_fd(),
                events: POLLIN,
                returned: 0,
            })
            .collect();
        let count =
            c_ulong::try_from(descriptors.len()).map_err(|error| HardwareError::Failed {
                device: "input",
                reason: error.to_string(),
            })?;
        let milliseconds = c_int::try_from(timeout.as_millis()).unwrap_or(c_int::MAX);
        let ready = unsafe { poll(descriptors.as_mut_ptr(), count, milliseconds) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(EINTR) {
                return Ok(());
            }
            return Err(failed("input", &error));
        }
        let present = self
            .devices
            .iter_mut()
            .filter(|device| device.file.is_some());
        let mut outcome = Ok(());
        for (device, descriptor) in present.zip(&descriptors) {
            if descriptor.returned & (POLLIN | POLLERR | POLLHUP | POLLNVAL) != 0 {
                outcome = outcome.and(device.read(&mut self.pending));
            }
        }
        outcome
    }
}

impl Input for EvdevInput {
    fn wait(&mut self, seconds: u64) -> Result<Option<InputEvent>, HardwareError> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            self.scan();
            let mut timeout = deadline - now;
            if self.devices.iter().any(|device| device.file.is_none()) {
                timeout = timeout.min(RESCAN);
            }
            self.poll(timeout)?;
        }
    }
}

fn odm() -> Option<String> {
    fs::read_dir(BLOCKS).ok()?.flatten().find_map(|entry| {
        partition(
            &fs::read_to_string(entry.path().join("uevent")).ok()?,
            PARTITION,
        )
    })
}

fn text(value: &str) -> Result<CString, HardwareError> {
    CString::new(value).map_err(|error| HardwareError::Failed {
        device: "odm",
        reason: error.to_string(),
    })
}

#[derive(Debug)]
pub struct Answer {
    pub name: String,
    pub outcome: Result<(), HardwareError>,
}

#[derive(Debug)]
pub struct FirmwareLoader {
    root: PathBuf,
}

impl FirmwareLoader {
    pub fn mount() -> Result<Self, HardwareError> {
        let device = odm().ok_or(HardwareError::Absent("odm"))?;
        fs::create_dir_all(MOUNT_POINT).map_err(|error| failed("odm", &error))?;
        let source = text(&format!("/dev/{device}"))?;
        let target = text(MOUNT_POINT)?;
        let result = unsafe {
            mount(
                source.as_ptr(),
                target.as_ptr(),
                c"ext4".as_ptr(),
                MS_RDONLY,
                std::ptr::null(),
            )
        };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(EBUSY) {
                return Err(failed("odm", &error));
            }
        }
        Ok(Self {
            root: Path::new(MOUNT_POINT).join(DIRECTORY),
        })
    }

    pub fn answer_pending(&self) -> Result<Vec<Answer>, HardwareError> {
        let entries = fs::read_dir(REQUESTS).map_err(|error| failed("firmware", &error))?;
        Ok(entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_owned();
                let file = requested(&name)?;
                let outcome = self.answer(&entry.path(), &file);
                Some(Answer { name, outcome })
            })
            .collect())
    }

    fn answer(&self, request: &Path, file: &Path) -> Result<(), HardwareError> {
        let loading = request.join("loading");
        let written = |path: &Path, bytes: &[u8]| {
            fs::write(path, bytes).map_err(|error| failed("firmware", &error))
        };
        match fs::read(self.root.join(file)) {
            Ok(bytes) => {
                written(&loading, b"1")?;
                written(&request.join("data"), &bytes)?;
                written(&loading, b"0")
            }
            Err(error) => {
                written(&loading, b"-1")?;
                Err(failed("firmware", &error))
            }
        }
    }
}
