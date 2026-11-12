use std::ffi::{c_int, c_void};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr;

use alarm_core::face::{Frame, Size};
use alarm_core::hardware::{Display, HardwareError};

use crate::{BACKLIGHT, DEVICE, FIX_SIZE, Mode, VAR_SIZE};

const FBIOGET_VSCREENINFO: c_int = 0x4600;
const FBIOGET_FSCREENINFO: c_int = 0x4602;
const FBIOPAN_DISPLAY: c_int = 0x4606;
const PROT_READ_WRITE: c_int = 0x3;
const MAP_SHARED: c_int = 0x1;

unsafe extern "C" {
    fn ioctl(descriptor: c_int, request: c_int, ...) -> c_int;
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: c_int,
        flags: c_int,
        descriptor: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(address: *mut c_void, length: usize) -> c_int;
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

fn control(file: &File, request: c_int, bytes: &mut [u8]) -> Result<(), HardwareError> {
    let result = unsafe { ioctl(file.as_raw_fd(), request, bytes.as_mut_ptr()) };
    if result < 0 {
        return Err(failed("fb0", &io::Error::last_os_error()));
    }
    Ok(())
}

fn mapped(file: &File, length: usize) -> Result<*mut u8, HardwareError> {
    let address = unsafe {
        mmap(
            ptr::null_mut(),
            length,
            PROT_READ_WRITE,
            MAP_SHARED,
            file.as_raw_fd(),
            0,
        )
    };
    if address.is_null() || address as isize == -1 {
        return Err(failed("fb0", &io::Error::last_os_error()));
    }
    Ok(address.cast())
}

#[derive(Debug)]
pub struct Panel {
    file: File,
    var: [u8; VAR_SIZE],
    mode: Mode,
    memory: *mut u8,
    shown: usize,
    brightness: String,
    lit: bool,
}

impl Panel {
    pub fn open(brightness: u32) -> Result<Self, HardwareError> {
        if !Path::new(BACKLIGHT).is_file() {
            return Err(HardwareError::Absent("lcd_backlight0"));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(DEVICE)
            .map_err(|error| failed("fb0", &error))?;
        let mut var = [0; VAR_SIZE];
        let mut fix = [0; FIX_SIZE];
        control(&file, FBIOGET_VSCREENINFO, &mut var)?;
        control(&file, FBIOGET_FSCREENINFO, &mut fix)?;
        let mode = Mode::parse(&var, &fix)?;
        let memory = mapped(&file, mode.mapped_bytes())?;
        Ok(Self {
            file,
            var,
            mode,
            memory,
            shown: mode.shown,
            brightness: brightness.to_string(),
            lit: false,
        })
    }

    fn page(&mut self, page: usize) -> &mut [u8] {
        let length = self.mode.page_bytes();
        let start = (page % self.mode.pages) * length;
        unsafe { std::slice::from_raw_parts_mut(self.memory.add(start), length) }
    }
}

impl Display for Panel {
    fn size(&self) -> Size {
        self.mode.size
    }

    fn show(&mut self, frame: &Frame) -> Result<(), HardwareError> {
        let mode = self.mode;
        let next = mode.after(self.shown);
        mode.encode(frame, self.page(next))?;
        mode.pan(&mut self.var, next)?;
        control(&self.file, FBIOPAN_DISPLAY, &mut self.var)?;
        self.shown = next;
        if !self.lit {
            fs::write(BACKLIGHT, &self.brightness)
                .map_err(|error| failed("lcd_backlight0", &error))?;
            self.lit = true;
        }
        Ok(())
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        unsafe {
            munmap(self.memory.cast(), self.mode.mapped_bytes());
        }
    }
}
