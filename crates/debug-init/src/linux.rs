use std::ffi::CStr;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::Duration;

use pid1::kmsg;
use pid1::linux::{into_recovery, mounted, reaped, spawned_on};

use crate::gadget;
use crate::shell::{self, Next};

const MOUNTS: [(&CStr, &CStr); 5] = [
    (c"proc", c"/proc"),
    (c"sysfs", c"/sys"),
    (c"pstore", c"/sys/fs/pstore"),
    (c"configfs", c"/config"),
    (c"tmpfs", c"/mnt"),
];
const WAIT: u32 = 10;

fn log(kmsg: &mut File, line: &str) {
    let _ = kmsg.write_all(line.as_bytes());
}

fn mounts(kmsg: &mut File) {
    for (filesystem, target) in MOUNTS {
        let result = mounted(filesystem, target);
        let (filesystem, target) = (filesystem.to_string_lossy(), target.to_string_lossy());
        match result {
            Ok(()) => log(
                kmsg,
                &kmsg::line(&format!("debug: mounted {filesystem} on {target}")),
            ),
            Err(error) => log(
                kmsg,
                &kmsg::failure(&format!("mounting {filesystem} on {target}"), &error),
            ),
        }
    }
}

fn appeared(kmsg: &mut File, path: &str) -> bool {
    for _ in 0..WAIT {
        if Path::new(path).exists() {
            return true;
        }
        thread::sleep(Duration::from_secs(1));
    }
    let found = Path::new(path).exists();
    if !found {
        log(
            kmsg,
            &kmsg::line(&format!("debug: {path} did not appear in {WAIT} s")),
        );
    }
    found
}

fn served(kmsg: &mut File) -> bool {
    if !appeared(kmsg, gadget::UDC_CLASS) {
        return false;
    }
    for step in gadget::plan(Path::new("/config")) {
        if let Err(error) = step.apply() {
            log(kmsg, &kmsg::failure(&step.to_string(), &error));
            return false;
        }
    }
    log(
        kmsg,
        &kmsg::line(&format!("debug: gadget bound to {}", gadget::UDC)),
    );
    appeared(kmsg, gadget::TTY)
}

pub fn run() -> ! {
    let Ok(mut kmsg) = mounted(c"devtmpfs", c"/dev").and_then(|()| pid1::linux::kmsg()) else {
        into_recovery(None)
    };
    log(&mut kmsg, &kmsg::line(crate::MARKER));
    mounts(&mut kmsg);
    if !served(&mut kmsg) {
        into_recovery(Some(&mut kmsg));
    }
    loop {
        let status = spawned_on(Path::new(gadget::TTY), shell::PROGRAM, shell::PATH)
            .inspect(|child| {
                log(
                    &mut kmsg,
                    &kmsg::line(&format!(
                        "debug: {} started as {}",
                        shell::PROGRAM,
                        child.id()
                    )),
                );
            })
            .and_then(|child| reaped(&child));
        match status {
            Ok(status) => {
                log(&mut kmsg, &shell::ended(status));
                if shell::next(status) == Next::Recovery {
                    into_recovery(Some(&mut kmsg));
                }
                thread::sleep(shell::PAUSE);
            }
            Err(error) => {
                log(
                    &mut kmsg,
                    &kmsg::failure(&format!("running {}", shell::PROGRAM), &error),
                );
                into_recovery(Some(&mut kmsg));
            }
        }
    }
}
