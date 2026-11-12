use alarm_core::face::{Frame, Size};
use alarm_core::hardware::HardwareError;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const DEVICE: &str = "/dev/fb0";
pub const BACKLIGHT: &str = "/sys/class/leds/lcd_backlight0/brightness";
pub const VAR_SIZE: usize = 160;
pub const FIX_SIZE: usize = 80;

const NAME: &str = "fb0";
const OPAQUE: u32 = 0xff00_0000;
const BYTES_PER_PIXEL: usize = 4;
const PACKED_PIXELS: u32 = 0;
const TRUECOLOR: u32 = 2;
const XRES: usize = 0;
const YRES: usize = 1;
const YRES_VIRTUAL: usize = 3;
const XOFFSET: usize = 4;
const YOFFSET: usize = 5;
const BITS_PER_PIXEL: usize = 6;
const RED: usize = 8;
const GREEN: usize = 11;
const BLUE: usize = 14;
const SMEM_LEN: usize = 24;
const TYPE: usize = 28;
const VISUAL: usize = 36;
const LINE_LENGTH: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    pub size: Size,
    pub pages: usize,
    pub shown: usize,
    pub line_length: usize,
}

fn failed(reason: String) -> HardwareError {
    HardwareError::Failed {
        device: NAME,
        reason,
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, HardwareError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| failed(format!("screeninfo ends before byte {offset}")))
}

fn count(bytes: &[u8], offset: usize) -> Result<usize, HardwareError> {
    usize::try_from(word(bytes, offset)?).map_err(|error| failed(error.to_string()))
}

fn var_word(var: &[u8], index: usize) -> Result<u32, HardwareError> {
    word(var, index * 4)
}

fn var_count(var: &[u8], index: usize) -> Result<usize, HardwareError> {
    count(var, index * 4)
}

fn channel(var: &[u8], index: usize) -> Result<(u32, u32), HardwareError> {
    Ok((var_word(var, index)?, var_word(var, index + 1)?))
}

impl Mode {
    pub fn parse(var: &[u8], fix: &[u8]) -> Result<Self, HardwareError> {
        if var.len() != VAR_SIZE || fix.len() != FIX_SIZE {
            return Err(failed(format!(
                "screeninfo is {} and {} bytes, not {VAR_SIZE} and {FIX_SIZE}",
                var.len(),
                fix.len()
            )));
        }
        let bits = var_word(var, BITS_PER_PIXEL)?;
        let channels = [
            channel(var, RED)?,
            channel(var, GREEN)?,
            channel(var, BLUE)?,
        ];
        if bits != 32 || channels != [(16, 8), (8, 8), (0, 8)] {
            return Err(failed(format!(
                "pixels are {bits} bpp with r/g/b at {channels:?}, not 32 bpp XRGB8888"
            )));
        }
        let (kind, visual) = (word(fix, TYPE)?, word(fix, VISUAL)?);
        if kind != PACKED_PIXELS || visual != TRUECOLOR {
            return Err(failed(format!(
                "type {kind} and visual {visual} are not packed truecolor"
            )));
        }
        let size = Size {
            width: var_count(var, XRES)?,
            height: var_count(var, YRES)?,
        };
        let line_length = count(fix, LINE_LENGTH)?;
        let pages = var_count(var, YRES_VIRTUAL)? / size.height.max(1);
        let mode = Self {
            size,
            pages,
            shown: var_count(var, YOFFSET)? / size.height.max(1),
            line_length,
        };
        if size.width == 0 || size.height == 0 || pages == 0 {
            return Err(failed(format!("a {size:?} panel with {pages} pages")));
        }
        if line_length < size.width * BYTES_PER_PIXEL {
            return Err(failed(format!(
                "a line of {line_length} bytes cannot hold {} pixels",
                size.width
            )));
        }
        if count(fix, SMEM_LEN)? < mode.mapped_bytes() {
            return Err(failed(format!(
                "{} bytes of memory cannot hold {pages} pages",
                count(fix, SMEM_LEN)?
            )));
        }
        Ok(mode)
    }

    #[must_use]
    pub fn page_bytes(&self) -> usize {
        self.line_length * self.size.height
    }

    #[must_use]
    pub fn mapped_bytes(&self) -> usize {
        self.page_bytes() * self.pages
    }

    #[must_use]
    pub fn after(&self, page: usize) -> usize {
        (page + 1) % self.pages
    }

    pub fn pan(&self, var: &mut [u8], page: usize) -> Result<(), HardwareError> {
        let yoffset =
            u32::try_from(page * self.size.height).map_err(|error| failed(error.to_string()))?;
        for (index, value) in [(XOFFSET, 0), (YOFFSET, yoffset)] {
            var.get_mut(index * 4..index * 4 + 4)
                .ok_or_else(|| failed(format!("screeninfo ends before word {index}")))?
                .copy_from_slice(&value.to_le_bytes());
        }
        Ok(())
    }

    pub fn encode(&self, frame: &Frame, page: &mut [u8]) -> Result<(), HardwareError> {
        if frame.size() != self.size || page.len() < self.page_bytes() {
            return Err(failed(format!(
                "a {:?} frame does not fit a {:?} page of {} bytes",
                frame.size(),
                self.size,
                page.len()
            )));
        }
        for (y, line) in page.chunks_exact_mut(self.line_length).enumerate() {
            let (pixels, _) = line.as_chunks_mut::<BYTES_PER_PIXEL>();
            for (pixel, colour) in pixels.iter_mut().zip(frame.row(y)) {
                *pixel = (OPAQUE | colour.0).to_le_bytes();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alarm_core::face::{BACKGROUND, Colour, FOREGROUND};

    use super::*;

    const VAR: &[u8] = include_bytes!("../testdata/var_screeninfo");
    const FIX: &[u8] = include_bytes!("../testdata/fix_screeninfo");

    fn panel() -> Result<Mode, HardwareError> {
        Mode::parse(VAR, FIX)
    }

    fn small() -> Mode {
        Mode {
            size: Size {
                width: 2,
                height: 2,
            },
            pages: 3,
            shown: 0,
            line_length: 12,
        }
    }

    fn set(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn the_recorded_panel_parses_to_its_mode() -> Result<(), HardwareError> {
        assert_eq!(
            panel()?,
            Mode {
                size: Size {
                    width: 1080,
                    height: 2280,
                },
                pages: 3,
                shown: 1,
                line_length: 4320,
            }
        );
        assert_eq!(panel()?.mapped_bytes(), 4320 * 6840);
        Ok(())
    }

    #[test]
    fn a_pixel_format_other_than_xrgb_is_refused() {
        let mut var = VAR.to_vec();
        set(&mut var, RED * 4, 0);
        set(&mut var, BLUE * 4, 16);
        assert!(matches!(
            Mode::parse(&var, FIX),
            Err(HardwareError::Failed { device: "fb0", .. })
        ));
        let mut shallow = VAR.to_vec();
        set(&mut shallow, BITS_PER_PIXEL * 4, 16);
        assert!(Mode::parse(&shallow, FIX).is_err());
    }

    #[test]
    fn a_short_line_or_memory_is_refused() {
        let mut fix = FIX.to_vec();
        set(&mut fix, LINE_LENGTH, 4316);
        assert!(Mode::parse(VAR, &fix).is_err());
        let mut memory = FIX.to_vec();
        set(&mut memory, SMEM_LEN, 4320 * 2280 * 2);
        assert!(Mode::parse(VAR, &memory).is_err());
    }

    #[test]
    fn truncated_screeninfo_is_refused() {
        assert!(Mode::parse(&VAR[..80], FIX).is_err());
        assert!(Mode::parse(VAR, &FIX[..40]).is_err());
    }

    #[test]
    fn pages_take_turns() -> Result<(), HardwareError> {
        let mode = panel()?;
        assert_eq!(mode.after(mode.shown), 2);
        assert_eq!(mode.after(2), 0);
        Ok(())
    }

    #[test]
    fn a_pan_moves_only_the_offsets() -> Result<(), HardwareError> {
        let mode = panel()?;
        let mut var = VAR.to_vec();
        mode.pan(&mut var, 2)?;
        assert_eq!(var_word(&var, YOFFSET)?, 4560);
        assert_eq!(var_word(&var, XOFFSET)?, 0);
        assert_eq!(var[..YOFFSET * 4], VAR[..YOFFSET * 4]);
        assert_eq!(var[YOFFSET * 4 + 4..], VAR[YOFFSET * 4 + 4..]);
        Ok(())
    }

    #[test]
    fn a_frame_is_written_opaque_and_little_endian_within_the_stride() -> Result<(), HardwareError>
    {
        let mode = small();
        let frame = Frame::filled(mode.size, FOREGROUND);
        let mut page = vec![0xaa; mode.page_bytes()];
        mode.encode(&frame, &mut page)?;
        assert_eq!(
            page,
            [
                [0xff, 0xff, 0xff, 0xff],
                [0xff, 0xff, 0xff, 0xff],
                [0xaa, 0xaa, 0xaa, 0xaa],
                [0xff, 0xff, 0xff, 0xff],
                [0xff, 0xff, 0xff, 0xff],
                [0xaa, 0xaa, 0xaa, 0xaa],
            ]
            .concat()
        );
        let red = Frame::filled(mode.size, Colour(0x12_3456));
        mode.encode(&red, &mut page)?;
        assert_eq!(page[..4], [0x56, 0x34, 0x12, 0xff]);
        Ok(())
    }

    #[test]
    fn a_frame_of_the_wrong_size_is_refused() {
        let mode = small();
        let frame = Frame::filled(
            Size {
                width: 3,
                height: 2,
            },
            BACKGROUND,
        );
        let mut page = vec![0; mode.page_bytes()];
        assert!(mode.encode(&frame, &mut page).is_err());
        assert!(
            mode.encode(&Frame::filled(mode.size, BACKGROUND), &mut page[..8])
                .is_err()
        );
    }
}
