use alarm_core::hardware::HardwareError;

pub mod control;
pub mod dsp;
pub mod pcm;
pub mod route;
pub mod wav;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const HIFI_MISC: &str = "/dev/hifi_misc";
pub const CONTROL: &str = "/dev/snd/controlC0";
pub const PCM: &str = "/dev/snd/pcmC0D0p";
pub const DSP_INIT: &str = "/system/etc/alarmd/dsp-init.bin";
pub const ROUTE: &str = "/system/etc/alarmd/speaker-route.tsv";
pub const SOUND: &str = "/system/media/audio/alarms/ringtone.wav";

fn failed(device: &'static str, reason: String) -> HardwareError {
    HardwareError::Failed { device, reason }
}

fn word(device: &'static str, bytes: &[u8], at: usize) -> Result<u32, HardwareError> {
    bytes
        .get(at..at + 4)
        .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| failed(device, format!("ends before byte {}", at + 4)))
}

fn put(bytes: &mut [u8], at: usize, value: &[u8]) {
    if let Some(slot) = bytes.get_mut(at..at + value.len()) {
        slot.copy_from_slice(value);
    }
}

#[cfg(test)]
pub(crate) fn encoded(direction: u32, kind: u8, number: u8, size: usize) -> Option<u32> {
    let size = u32::try_from(size).ok().filter(|size| *size < 1 << 14)?;
    Some((direction << 30) | (size << 16) | (u32::from(kind) << 8) | u32::from(number))
}
