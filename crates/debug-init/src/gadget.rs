use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

pub const UDC: &str = "hisi-usb-otg";
pub const UDC_CLASS: &str = "/sys/class/udc/hisi-usb-otg";
pub const TTY: &str = "/dev/ttyGS0";

const VENDOR: &str = "0x1d6b";
const PRODUCT: &str = "0x0104";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Directory(PathBuf),
    Write(PathBuf, &'static str),
    Link(PathBuf, PathBuf),
}

impl Step {
    pub fn apply(&self) -> io::Result<()> {
        match self {
            Step::Directory(path) => fs::create_dir_all(path),
            Step::Write(path, value) => fs::write(path, value),
            Step::Link(original, link) => symlink(original, link),
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Directory(path) => write!(formatter, "creating {}", path.display()),
            Step::Write(path, value) => write!(formatter, "writing {value} to {}", path.display()),
            Step::Link(original, link) => write!(
                formatter,
                "linking {} to {}",
                link.display(),
                original.display()
            ),
        }
    }
}

#[must_use]
pub fn plan(configfs: &Path) -> Vec<Step> {
    let gadget = configfs.join("usb_gadget/debug");
    let strings = gadget.join("strings/0x409");
    let function = gadget.join("functions/acm.gs0");
    let config = gadget.join("configs/c.1");
    let config_strings = config.join("strings/0x409");
    vec![
        Step::Directory(gadget.clone()),
        Step::Write(gadget.join("idVendor"), VENDOR),
        Step::Write(gadget.join("idProduct"), PRODUCT),
        Step::Write(gadget.join("bcdDevice"), "0x0100"),
        Step::Write(gadget.join("bcdUSB"), "0x0200"),
        Step::Directory(strings.clone()),
        Step::Write(strings.join("manufacturer"), "p20-alarm-os"),
        Step::Write(strings.join("product"), "debug shell"),
        Step::Write(strings.join("serialnumber"), "p20-alarm-os"),
        Step::Directory(function.clone()),
        Step::Directory(config_strings.clone()),
        Step::Write(config_strings.join("configuration"), "acm"),
        Step::Link(function, config.join("acm.gs0")),
        Step::Write(gadget.join("UDC"), UDC),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::process;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> io::Result<Self> {
            let path = env::temp_dir().join(format!("debug-init-{name}-{}", process::id()));
            if path.exists() {
                fs::remove_dir_all(&path)?;
            }
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_gadget_is_the_linux_foundations_composite_with_one_acm_function() -> io::Result<()> {
        let scratch = Scratch::new("ids")?;
        for step in plan(&scratch.0) {
            step.apply()?;
        }
        let gadget = scratch.0.join("usb_gadget/debug");
        assert_eq!(fs::read_to_string(gadget.join("idVendor"))?, "0x1d6b");
        assert_eq!(fs::read_to_string(gadget.join("idProduct"))?, "0x0104");
        assert_eq!(fs::read_dir(gadget.join("functions"))?.count(), 1);
        assert_eq!(
            fs::read_link(gadget.join("configs/c.1/acm.gs0"))?,
            gadget.join("functions/acm.gs0")
        );
        assert_eq!(fs::read_to_string(gadget.join("UDC"))?, UDC);
        Ok(())
    }

    #[test]
    fn the_udc_is_bound_last_once_the_function_is_linked() {
        let steps = plan(Path::new("/config"));
        let link = steps.iter().position(|step| matches!(step, Step::Link(..)));
        let bind = steps.iter().position(|step| {
            *step == Step::Write(PathBuf::from("/config/usb_gadget/debug/UDC"), UDC)
        });
        assert_eq!(bind, Some(steps.len() - 1));
        assert!(link.is_some_and(|link| link < steps.len() - 1));
    }

    #[test]
    fn a_step_says_what_it_does() {
        let steps = plan(Path::new("/config"));
        assert_eq!(
            steps.last().map(ToString::to_string),
            Some("writing hisi-usb-otg to /config/usb_gadget/debug/UDC".to_owned())
        );
        assert!(steps.iter().any(|step| step.to_string()
            == "linking /config/usb_gadget/debug/configs/c.1/acm.gs0 to /config/usb_gadget/debug/functions/acm.gs0"));
    }
}
