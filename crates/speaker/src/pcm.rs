use alarm_core::hardware::HardwareError;

use crate::wav::{BITS, CHANNELS, FRAME_BYTES, RATE};
use crate::{failed, put, word};

pub const HW_PARAMS: u32 = 0xc260_4111;
pub const PREPARE: u32 = 0x4140;
pub const DROP: u32 = 0x4143;
pub const WRITEI_FRAMES: u32 = 0x4018_4150;
pub const HW_PARAMS_BYTES: usize = 608;
pub const XFERI_BYTES: usize = 24;
pub const PERIOD_FRAMES: u32 = 960;
pub const PERIODS: u32 = 2;

pub(crate) const DEVICE: &str = "pcmC0D0p";
const MASK_BYTES: usize = 32;
const MASKS_AT: usize = 4;
const MASKS: usize = 3;
const INTERVAL_BYTES: usize = 12;
const INTERVALS_AT: usize = MASKS_AT + 8 * MASK_BYTES;
const FIRST_INTERVAL: usize = 8;
const INTERVALS: usize = 12;
const RMASK: usize = 512;
const INFO: usize = 520;
const ACCESS: usize = 0;
const FORMAT: usize = 1;
const SUBFORMAT: usize = 2;
const SAMPLE_BITS: usize = 8;
const FRAME_BITS: usize = 9;
const CHANNELS_INTERVAL: usize = 10;
const RATE_INTERVAL: usize = 11;
const PERIOD_SIZE: usize = 13;
const PERIODS_INTERVAL: usize = 15;
const RW_INTERLEAVED: u32 = 3;
const S16_LE: u32 = 2;
const STANDARD: u32 = 0;
const INTEGER: u32 = 1 << 2;

fn interval_at(index: usize) -> usize {
    INTERVALS_AT + (index - FIRST_INTERVAL) * INTERVAL_BYTES
}

fn mask(bytes: &mut [u8], index: usize, bit: u32) {
    let at = MASKS_AT + index * MASK_BYTES;
    put(bytes, at, &[0; MASK_BYTES]);
    let word = usize::try_from(bit / 32).unwrap_or(0);
    put(bytes, at + word * 4, &(1_u32 << (bit % 32)).to_le_bytes());
}

fn exactly(bytes: &mut [u8], index: usize, value: u32) {
    let at = interval_at(index);
    put(bytes, at, &value.to_le_bytes());
    put(bytes, at + 4, &value.to_le_bytes());
    put(bytes, at + 8, &INTEGER.to_le_bytes());
}

#[must_use]
pub fn hw_params() -> Vec<u8> {
    let mut bytes = vec![0; HW_PARAMS_BYTES];
    for index in 0..MASKS {
        put(
            &mut bytes,
            MASKS_AT + index * MASK_BYTES,
            &[0xff; MASK_BYTES],
        );
    }
    for index in FIRST_INTERVAL..FIRST_INTERVAL + INTERVALS {
        put(&mut bytes, interval_at(index) + 4, &u32::MAX.to_le_bytes());
    }
    mask(&mut bytes, ACCESS, RW_INTERLEAVED);
    mask(&mut bytes, FORMAT, S16_LE);
    mask(&mut bytes, SUBFORMAT, STANDARD);
    exactly(&mut bytes, SAMPLE_BITS, BITS);
    exactly(&mut bytes, FRAME_BITS, BITS * CHANNELS);
    exactly(&mut bytes, CHANNELS_INTERVAL, CHANNELS);
    exactly(&mut bytes, RATE_INTERVAL, RATE);
    exactly(&mut bytes, PERIOD_SIZE, PERIOD_FRAMES);
    exactly(&mut bytes, PERIODS_INTERVAL, PERIODS);
    put(&mut bytes, RMASK, &u32::MAX.to_le_bytes());
    put(&mut bytes, INFO, &u32::MAX.to_le_bytes());
    bytes
}

pub fn refined(bytes: &[u8]) -> Result<(), HardwareError> {
    for (index, name, wanted) in [
        (CHANNELS_INTERVAL, "channels", CHANNELS),
        (RATE_INTERVAL, "rate", RATE),
        (PERIOD_SIZE, "period", PERIOD_FRAMES),
        (PERIODS_INTERVAL, "periods", PERIODS),
    ] {
        let at = interval_at(index);
        let (min, max) = (word(DEVICE, bytes, at)?, word(DEVICE, bytes, at + 4)?);
        if (min, max) != (wanted, wanted) {
            return Err(failed(
                DEVICE,
                format!("{name} refined to {min}..{max}, not {wanted}"),
            ));
        }
    }
    Ok(())
}

#[must_use]
pub fn xferi(buffer: u64, frames: u64) -> [u8; XFERI_BYTES] {
    let mut bytes = [0; XFERI_BYTES];
    put(&mut bytes, 8, &buffer.to_le_bytes());
    put(&mut bytes, 16, &frames.to_le_bytes());
    bytes
}

#[must_use]
pub fn transferred(bytes: &[u8; XFERI_BYTES]) -> i64 {
    let mut result = [0; 8];
    result.copy_from_slice(&bytes[..8]);
    i64::from_le_bytes(result)
}

#[must_use]
pub fn period(samples: &[u8], at: usize) -> &[u8] {
    let frames = usize::try_from(PERIOD_FRAMES).unwrap_or(0);
    let end = samples.len().min(at.saturating_add(frames * FRAME_BYTES));
    samples.get(at..end).unwrap_or_default()
}

#[must_use]
pub fn next(samples: &[u8], at: usize, frames: usize) -> usize {
    match samples.len() {
        0 => 0,
        length => (at + frames * FRAME_BYTES) % length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoded;

    fn interval(bytes: &[u8], index: usize) -> Result<(u32, u32, u32), HardwareError> {
        let at = interval_at(index);
        Ok((
            word(DEVICE, bytes, at)?,
            word(DEVICE, bytes, at + 4)?,
            word(DEVICE, bytes, at + 8)?,
        ))
    }

    #[test]
    fn the_requests_encode_the_layouts_of_their_structs() {
        assert_eq!(encoded(3, b'A', 0x11, HW_PARAMS_BYTES), Some(HW_PARAMS));
        assert_eq!(encoded(0, b'A', 0x40, 0), Some(PREPARE));
        assert_eq!(encoded(0, b'A', 0x43, 0), Some(DROP));
        assert_eq!(encoded(1, b'A', 0x50, XFERI_BYTES), Some(WRITEI_FRAMES));
        assert_eq!(
            interval_at(FIRST_INTERVAL + INTERVALS) + 9 * INTERVAL_BYTES,
            RMASK
        );
        assert_eq!(RMASK + 6 * 4 + 8 + 64, HW_PARAMS_BYTES);
    }

    #[test]
    fn the_parameters_ask_for_16_bit_48_khz_stereo_in_periods_of_960() -> Result<(), HardwareError>
    {
        let bytes = hw_params();
        assert_eq!(bytes.len(), HW_PARAMS_BYTES);
        assert_eq!(word(DEVICE, &bytes, MASKS_AT)?, 1 << RW_INTERLEAVED);
        assert_eq!(word(DEVICE, &bytes, MASKS_AT + 4)?, 0);
        assert_eq!(word(DEVICE, &bytes, MASKS_AT + MASK_BYTES)?, 1 << S16_LE);
        assert_eq!(word(DEVICE, &bytes, MASKS_AT + 2 * MASK_BYTES)?, 1);
        assert_eq!(word(DEVICE, &bytes, MASKS_AT + 3 * MASK_BYTES)?, 0);
        assert_eq!(interval(&bytes, SAMPLE_BITS)?, (16, 16, INTEGER));
        assert_eq!(interval(&bytes, FRAME_BITS)?, (32, 32, INTEGER));
        assert_eq!(interval(&bytes, CHANNELS_INTERVAL)?, (2, 2, INTEGER));
        assert_eq!(interval(&bytes, RATE_INTERVAL)?, (48_000, 48_000, INTEGER));
        assert_eq!(interval(&bytes, PERIOD_SIZE)?, (960, 960, INTEGER));
        assert_eq!(interval(&bytes, PERIODS_INTERVAL)?, (2, 2, INTEGER));
        assert_eq!(interval(&bytes, 12)?, (0, u32::MAX, 0));
        assert_eq!(interval(&bytes, 19)?, (0, u32::MAX, 0));
        assert_eq!(word(DEVICE, &bytes, RMASK)?, u32::MAX);
        assert_eq!(word(DEVICE, &bytes, RMASK + 4)?, 0);
        assert_eq!(word(DEVICE, &bytes, INFO)?, u32::MAX);
        Ok(())
    }

    #[test]
    fn the_kernel_must_keep_the_asked_rate_channels_and_period() -> Result<(), HardwareError> {
        let mut bytes = hw_params();
        refined(&bytes)?;
        put(&mut bytes, interval_at(PERIOD_SIZE), &480_u32.to_le_bytes());
        assert!(matches!(
            refined(&bytes),
            Err(HardwareError::Failed { device: DEVICE, .. })
        ));
        Ok(())
    }

    #[test]
    fn a_transfer_names_the_buffer_and_its_frames_and_reads_back_the_result() {
        let mut bytes = xferi(0x71_9b51_e080, 960);
        assert_eq!(bytes[8..16], 0x71_9b51_e080_u64.to_le_bytes());
        assert_eq!(bytes[16..24], 960_u64.to_le_bytes());
        assert_eq!(transferred(&bytes), 0);
        bytes[..8].copy_from_slice(&512_i64.to_le_bytes());
        assert_eq!(transferred(&bytes), 512);
    }

    #[test]
    fn the_sound_is_written_a_period_at_a_time_and_loops() {
        let samples = vec![0; 2_000 * FRAME_BYTES];
        assert_eq!(period(&samples, 0).len(), 960 * FRAME_BYTES);
        let at = next(&samples, 0, 960);
        let at = next(&samples, at, 960);
        assert_eq!(at, 1_920 * FRAME_BYTES);
        assert_eq!(period(&samples, at).len(), 80 * FRAME_BYTES);
        assert_eq!(next(&samples, at, 80), 0);
        assert_eq!(next(&samples, at, 40), 1_960 * FRAME_BYTES);
        assert_eq!(period(&samples, samples.len() + 4), []);
    }
}
