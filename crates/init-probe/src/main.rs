#![cfg_attr(all(target_os = "linux", target_arch = "aarch64"), no_main)]

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> std::ffi::c_int {
    init_probe::linux::run();
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "init-probe runs only as /init on the phone, where it logs {:?}",
        init_probe::marker::MARKER
    );
    std::process::ExitCode::FAILURE
}
