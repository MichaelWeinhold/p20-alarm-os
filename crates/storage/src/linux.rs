use std::ffi::c_int;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;

const POSIX_FADV_DONTNEED: c_int = 4;

unsafe extern "C" {
    fn posix_fadvise(descriptor: c_int, offset: i64, length: i64, advice: c_int) -> c_int;
}

pub fn uncached(file: &File) -> io::Result<()> {
    let result = unsafe { posix_fadvise(file.as_raw_fd(), 0, 0, POSIX_FADV_DONTNEED) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result))
    }
}
