#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn main() {
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
