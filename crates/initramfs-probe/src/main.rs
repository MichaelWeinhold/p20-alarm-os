#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn main() {
    initramfs_probe::linux::run();
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "initramfs-probe runs only as /init on the phone, where it logs {:?}",
        initramfs_probe::marker::MARKER
    );
    std::process::ExitCode::FAILURE
}
