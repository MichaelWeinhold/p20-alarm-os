use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::{c_int, c_ulong};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use alarm_core::hardware::{HardwareError, Speaker};
use alarm_core::sound::{UNITY_GAIN, Volume};

use crate::control::{self, ELEM_INFO, ELEM_LIST, ELEM_WRITE, ENUMERATED, ID_BYTES, Info, Write};
use crate::dsp::{self, Message};
use crate::pcm::{self, DROP, HW_FREE, HW_PARAMS, PREPARE, WRITEI_FRAMES};
use crate::route::{self, Control, Route};
use crate::wav::{self, FRAME_BYTES};
use crate::{CONTROL, DSP_INIT, HIFI_MISC, PCM, ROUTE, SOUND};

const HIFI: &str = "hifi_misc";
const MOST_ELEMENTS: u32 = 4_096;
const SMALLEST_REPLY: usize = 4;
const EINTR: i32 = 4;
const EPIPE: i32 = 32;
const ESTRPIPE: i32 = 86;
const SETTLE: Duration = Duration::from_secs(5);

unsafe extern "C" {
    fn ioctl(descriptor: c_int, request: c_int, ...) -> c_int;
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

fn refused(device: &'static str, reason: String) -> HardwareError {
    HardwareError::Failed { device, reason }
}

fn address(bytes: &mut [u8]) -> u64 {
    bytes.as_mut_ptr().expose_provenance() as u64
}

fn source(bytes: &[u8]) -> u64 {
    bytes.as_ptr().expose_provenance() as u64
}

fn requested(
    file: &File,
    device: &'static str,
    request: u32,
    argument: &mut [u8],
) -> Result<(), HardwareError> {
    let size = usize::try_from((request >> 16) & 0x3fff).unwrap_or(usize::MAX);
    if argument.len() != size {
        return Err(refused(
            device,
            format!("{request:#x} takes {size} bytes, not {}", argument.len()),
        ));
    }
    let result = unsafe {
        ioctl(
            file.as_raw_fd(),
            request.cast_signed(),
            argument.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(failed(device, &io::Error::last_os_error()));
    }
    Ok(())
}

fn plain(file: &File, request: u32) -> io::Result<()> {
    let nothing: c_ulong = 0;
    let result = unsafe { ioctl(file.as_raw_fd(), request.cast_signed(), nothing) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn sent(hifi: &File, message: &Message) -> Result<(), HardwareError> {
    let length =
        u32::try_from(message.body.len()).map_err(|error| refused(HIFI, error.to_string()))?;
    let reply_length =
        usize::try_from(message.reply).map_err(|error| refused(HIFI, error.to_string()))?;
    let mut reply = vec![0; reply_length.max(SMALLEST_REPLY)];
    let mut argument = dsp::argument(
        message.kind,
        source(&message.body),
        length,
        address(&mut reply),
        message.reply,
    );
    let result = unsafe {
        ioctl(
            hifi.as_raw_fd(),
            message.kind.request().cast_signed(),
            argument.as_mut_ptr(),
        )
    };
    if result < 0 {
        let error = io::Error::last_os_error();
        return Err(refused(
            HIFI,
            format!("{:?} message of {length} bytes: {error}", message.kind),
        ));
    }
    Ok(())
}

fn listed_ids(control: &File, space: u32) -> Result<(Vec<u8>, u32, u32), HardwareError> {
    let bytes = usize::try_from(space)
        .map_err(|error| refused(control::DEVICE, error.to_string()))?
        * ID_BYTES;
    let mut ids = vec![0; bytes];
    let mut list = control::list(space, address(&mut ids));
    let result = unsafe {
        ioctl(
            control.as_raw_fd(),
            ELEM_LIST.cast_signed(),
            list.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(failed(control::DEVICE, &io::Error::last_os_error()));
    }
    let (used, count) = control::listed(&list)?;
    Ok((ids, used, count))
}

fn elements(control: &File) -> Result<HashMap<String, u32>, HardwareError> {
    let (_, _, count) = listed_ids(control, 0)?;
    if count > MOST_ELEMENTS {
        return Err(refused(control::DEVICE, format!("{count} elements listed")));
    }
    let (mut ids, used, _) = listed_ids(control, count)?;
    if used > count {
        return Err(refused(
            control::DEVICE,
            format!("{used} elements sent into space for {count}"),
        ));
    }
    ids.truncate(usize::try_from(used).unwrap_or(0) * ID_BYTES);
    Ok(control::elements(&ids)?
        .into_iter()
        .rev()
        .map(|element| (element.name, element.numid))
        .collect())
}

fn described(control: &File, numid: u32, name: &str) -> Result<(Info, Vec<String>), HardwareError> {
    let mut asked = control::info(numid, 0);
    requested(control, control::DEVICE, ELEM_INFO, &mut asked)?;
    let info = control::described(name, &asked)?;
    let mut items = Vec::new();
    if info.kind == ENUMERATED {
        for item in 0..info.items {
            let mut asked = control::info(numid, item);
            requested(control, control::DEVICE, ELEM_INFO, &mut asked)?;
            items.push(control::item_name(&asked)?);
        }
    }
    Ok((info, items))
}

#[derive(Debug)]
struct Writes {
    ring: Vec<Write>,
    after: Vec<Write>,
}

fn resolved(control: &File, route: &Route) -> Result<Writes, HardwareError> {
    let numbers = elements(control)?;
    let mut infos = HashMap::new();
    let mut write = |wanted: &Control| -> Result<Write, HardwareError> {
        let numid = *numbers
            .get(&wanted.name)
            .ok_or_else(|| refused(control::DEVICE, format!("no element named {}", wanted.name)))?;
        let (info, items) = match infos.entry(numid) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(described(control, numid, &wanted.name)?),
        };
        Ok(Write {
            numid,
            count: info.count,
            setting: control::setting(&wanted.name, info, items, &wanted.value)?,
        })
    };
    let ring = route
        .ring
        .iter()
        .map(&mut write)
        .collect::<Result<_, _>>()?;
    let after = route
        .after
        .iter()
        .map(&mut write)
        .collect::<Result<_, _>>()?;
    Ok(Writes { ring, after })
}

fn set(control: &File, writes: &[Write]) -> Result<(), HardwareError> {
    for write in writes {
        let mut bytes = control::value(write);
        requested(control, control::DEVICE, ELEM_WRITE, &mut bytes).map_err(|error| {
            refused(
                control::DEVICE,
                format!("writing element {}: {error}", write.numid),
            )
        })?;
    }
    Ok(())
}

fn prepared(pcm: &File) -> Result<(), HardwareError> {
    let mut params = pcm::hw_params();
    requested(pcm, pcm::DEVICE, HW_PARAMS, &mut params)?;
    pcm::refined(&params)?;
    plain(pcm, PREPARE).map_err(|error| failed(pcm::DEVICE, &error))
}

fn written(pcm: &File, frames: &[u8]) -> io::Result<usize> {
    let count = (frames.len() / FRAME_BYTES) as u64;
    let mut transfer = pcm::xferi(source(frames), count);
    let result = unsafe {
        ioctl(
            pcm.as_raw_fd(),
            WRITEI_FRAMES.cast_signed(),
            transfer.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    usize::try_from(pcm::transferred(&transfer))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn played(
    pcm: &File,
    samples: &[u8],
    gain: &AtomicU32,
    stopping: &AtomicBool,
) -> Result<(), HardwareError> {
    let mut at = 0;
    let mut period = Vec::new();
    while !stopping.load(Ordering::Acquire) {
        pcm::scaled(
            pcm::period(samples, at),
            gain.load(Ordering::Relaxed),
            &mut period,
        );
        match written(pcm, &period) {
            Ok(0) => return Err(refused(pcm::DEVICE, "wrote no frames".to_owned())),
            Ok(frames) => at = pcm::next(samples, at, frames),
            Err(_) if stopping.load(Ordering::Acquire) => break,
            Err(error) if error.raw_os_error() == Some(EINTR) => {}
            Err(error) if matches!(error.raw_os_error(), Some(EPIPE | ESTRPIPE)) => {
                plain(pcm, PREPARE).map_err(|error| failed(pcm::DEVICE, &error))?;
            }
            Err(error) => return Err(failed(pcm::DEVICE, &error)),
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Playing {
    stopping: Arc<AtomicBool>,
    thread: JoinHandle<Result<(), HardwareError>>,
}

fn playing(
    pcm: Arc<File>,
    samples: Arc<[u8]>,
    gain: Arc<AtomicU32>,
) -> Result<Playing, HardwareError> {
    let stopping = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stopping);
    let thread = thread::Builder::new()
        .name("speaker".to_owned())
        .spawn(move || played(&pcm, &samples, &gain, &flag))
        .map_err(|error| failed(pcm::DEVICE, &error))?;
    Ok(Playing { stopping, thread })
}

fn read(path: &str, device: &'static str) -> Result<Vec<u8>, HardwareError> {
    fs::read(path).map_err(|error| failed(device, &error))
}

fn device(path: &str, device: &'static str) -> Result<File, HardwareError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| failed(device, &error))
}

pub struct AlsaSpeaker {
    writes: Writes,
    samples: Arc<[u8]>,
    gain: Arc<AtomicU32>,
    control: File,
    pcm: Arc<File>,
    replayed: Instant,
    playing: Option<Playing>,
}

impl fmt::Debug for AlsaSpeaker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AlsaSpeaker")
            .field("controls", &self.writes.ring.len())
            .field("sample_bytes", &self.samples.len())
            .field("playing", &self.playing.is_some())
            .finish_non_exhaustive()
    }
}

impl AlsaSpeaker {
    pub fn open() -> Result<Self, HardwareError> {
        let init = dsp::messages(&read(DSP_INIT, "dsp-init.bin")?)?;
        let route = String::from_utf8(read(ROUTE, "speaker-route.tsv")?)
            .map_err(|error| refused("speaker-route.tsv", error.to_string()))?;
        let route = route::parse(&route)?;
        let sound = read(SOUND, "alarm sound")?;
        let samples = Arc::from(wav::samples(&sound)?);
        let hifi = device(HIFI_MISC, HIFI)?;
        let control = device(CONTROL, control::DEVICE)?;
        let writes = resolved(&control, &route)?;
        for message in &init {
            sent(&hifi, message)?;
        }
        let replayed = Instant::now();
        let pcm = Arc::new(device(PCM, pcm::DEVICE)?);
        Ok(Self {
            writes,
            samples,
            gain: Arc::new(AtomicU32::new(UNITY_GAIN)),
            control,
            pcm,
            replayed,
            playing: None,
        })
    }
}

impl Speaker for AlsaSpeaker {
    fn start(&mut self, volume: Volume) -> Result<(), HardwareError> {
        self.gain.store(volume.gain(), Ordering::Relaxed);
        if self.playing.is_some() {
            return Ok(());
        }
        thread::sleep(SETTLE.saturating_sub(self.replayed.elapsed()));
        let started = set(&self.control, &self.writes.ring)
            .and_then(|()| prepared(&self.pcm))
            .and_then(|()| {
                playing(
                    Arc::clone(&self.pcm),
                    Arc::clone(&self.samples),
                    Arc::clone(&self.gain),
                )
            });
        match started {
            Ok(playing) => {
                self.playing = Some(playing);
                Ok(())
            }
            Err(error) => {
                let _ = set(&self.control, &self.writes.after);
                Err(error)
            }
        }
    }

    fn set_volume(&mut self, volume: Volume) -> Result<(), HardwareError> {
        self.gain.store(volume.gain(), Ordering::Relaxed);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), HardwareError> {
        let Some(playing) = self.playing.take() else {
            return Ok(());
        };
        playing.stopping.store(true, Ordering::Release);
        let dropped = plain(&self.pcm, DROP).map_err(|error| failed(pcm::DEVICE, &error));
        let played = playing
            .thread
            .join()
            .unwrap_or_else(|_| Err(refused(pcm::DEVICE, "the playing thread ended".to_owned())));
        let freed = plain(&self.pcm, HW_FREE).map_err(|error| failed(pcm::DEVICE, &error));
        let reset = set(&self.control, &self.writes.after);
        played.and(dropped).and(freed).and(reset)
    }
}
