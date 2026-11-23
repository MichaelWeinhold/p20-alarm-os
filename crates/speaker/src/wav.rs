use alarm_core::hardware::HardwareError;

use crate::{failed, word};

pub const HEADER_BYTES: usize = 44;
pub const RATE: u32 = 48_000;
pub const CHANNELS: u32 = 2;
pub const BITS: u32 = 16;
pub const FRAME_BYTES: usize = 4;

const DEVICE: &str = "alarm sound";
const PCM: u32 = 1;

fn tag(bytes: &[u8], at: usize, expected: &[u8]) -> Result<(), HardwareError> {
    match bytes.get(at..at + expected.len()) {
        Some(found) if found == expected => Ok(()),
        found => Err(failed(
            DEVICE,
            format!(
                "{found:?} at byte {at}, not {:?}",
                String::from_utf8_lossy(expected)
            ),
        )),
    }
}

fn expected(name: &str, found: u32, expected: u32) -> Result<(), HardwareError> {
    if found == expected {
        Ok(())
    } else {
        Err(failed(DEVICE, format!("{name} {found}, not {expected}")))
    }
}

fn half(bytes: &[u8], at: usize) -> Result<u32, HardwareError> {
    Ok(word(DEVICE, bytes, at)? & 0xffff)
}

pub fn samples(bytes: &[u8]) -> Result<&[u8], HardwareError> {
    if bytes.len() < HEADER_BYTES {
        return Err(failed(
            DEVICE,
            format!("{} bytes, shorter than a WAV header", bytes.len()),
        ));
    }
    let after_riff =
        u32::try_from(bytes.len() - 8).map_err(|error| failed(DEVICE, error.to_string()))?;
    tag(bytes, 0, b"RIFF")?;
    expected("RIFF length", word(DEVICE, bytes, 4)?, after_riff)?;
    tag(bytes, 8, b"WAVE")?;
    tag(bytes, 12, b"fmt ")?;
    expected("format length", word(DEVICE, bytes, 16)?, 16)?;
    expected("format", half(bytes, 20)?, PCM)?;
    expected("channels", half(bytes, 22)?, CHANNELS)?;
    expected("rate", word(DEVICE, bytes, 24)?, RATE)?;
    expected(
        "byte rate",
        word(DEVICE, bytes, 28)?,
        RATE * CHANNELS * BITS / 8,
    )?;
    expected("frame bytes", half(bytes, 32)?, CHANNELS * BITS / 8)?;
    expected("bits", half(bytes, 34)?, BITS)?;
    tag(bytes, 36, b"data")?;
    expected("data length", word(DEVICE, bytes, 40)?, after_riff - 36)?;
    let samples = &bytes[HEADER_BYTES..];
    if samples.is_empty() || !samples.len().is_multiple_of(FRAME_BYTES) {
        return Err(failed(
            DEVICE,
            format!("{} bytes of samples, not whole frames", samples.len()),
        ));
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(format: u16, channels: u16, rate: u32, bits: u16, samples: &[u8]) -> Vec<u8> {
        let frame = channels * bits / 8;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(
            &(36 + u32::try_from(samples.len()).unwrap_or(u32::MAX)).to_le_bytes(),
        );
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&format.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(frame)).to_le_bytes());
        bytes.extend_from_slice(&frame.to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(
            &u32::try_from(samples.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        bytes.extend_from_slice(samples);
        bytes
    }

    const FRAMES: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

    #[test]
    fn a_wav_of_16_bit_48_khz_stereo_gives_its_samples() -> Result<(), HardwareError> {
        let bytes = header(1, 2, 48_000, 16, &FRAMES);
        assert_eq!(bytes.len(), HEADER_BYTES + FRAMES.len());
        assert_eq!(samples(&bytes)?, FRAMES);
        Ok(())
    }

    #[test]
    fn any_other_format_rate_channel_count_or_width_is_a_hardware_error() {
        for bytes in [
            header(3, 2, 48_000, 16, &FRAMES),
            header(1, 1, 48_000, 16, &FRAMES),
            header(1, 2, 44_100, 16, &FRAMES),
            header(1, 2, 48_000, 24, &[0; 12]),
        ] {
            assert!(matches!(
                samples(&bytes),
                Err(HardwareError::Failed { device: DEVICE, .. })
            ));
        }
    }

    #[test]
    fn a_header_that_does_not_match_the_file_is_a_hardware_error() {
        let whole = header(1, 2, 48_000, 16, &FRAMES);
        let mut extended = header(1, 2, 48_000, 16, &FRAMES);
        extended.extend_from_slice(b"LIST");
        let mut renamed = whole.clone();
        renamed[0..4].copy_from_slice(b"RIFX");
        let mut chunked = whole.clone();
        chunked[36..40].copy_from_slice(b"LIST");
        for bytes in [
            &whole[..whole.len() - 4],
            &extended[..],
            &renamed[..],
            &chunked[..],
            &whole[..HEADER_BYTES - 1],
            &header(1, 2, 48_000, 16, &[])[..],
            &header(1, 2, 48_000, 16, &[0; 6])[..],
        ] {
            assert!(samples(bytes).is_err(), "{bytes:?}");
        }
    }
}
