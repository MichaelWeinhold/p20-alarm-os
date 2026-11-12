pub mod gadget;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;
pub mod shell;

pub const MARKER: &str = "debug /init ran from system";
