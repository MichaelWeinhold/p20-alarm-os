#![cfg_attr(all(target_os = "linux", target_arch = "aarch64"), no_main)]

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> std::ffi::c_int {
    debug_init::linux::run();
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "debug-init runs only as /init on the phone, where it logs {:?}",
        debug_init::MARKER
    );
    std::process::ExitCode::FAILURE
}
