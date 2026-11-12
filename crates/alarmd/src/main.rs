#![cfg_attr(all(target_os = "linux", target_arch = "aarch64"), no_main)]

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> std::ffi::c_int {
    alarmd::linux::run();
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn main() -> std::process::ExitCode {
    eprintln!("alarmd runs only as /init on the phone");
    std::process::ExitCode::FAILURE
}
