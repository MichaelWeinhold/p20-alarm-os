use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use alarm_core::hardware::{HardwareError, Settings};
use alarm_core::settings::{RECORD_BYTES, SLOT_BYTES, Slot};
use pid1::block::{BLOCKS, partition};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod linux;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use linux::uncached;

pub const PARTITION: &str = "reserved4";
pub const SIZE: u64 = 2_097_152;
pub const DEVICES: &str = "/dev";

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
#[allow(clippy::unnecessary_wraps)]
fn uncached(_file: &File) -> io::Result<()> {
    Ok(())
}

fn failed(error: &io::Error) -> HardwareError {
    if error.kind() == io::ErrorKind::NotFound {
        HardwareError::Absent(PARTITION)
    } else {
        HardwareError::Failed {
            device: PARTITION,
            reason: error.to_string(),
        }
    }
}

fn found(blocks: &Path) -> Result<String, HardwareError> {
    fs::read_dir(blocks)
        .map_err(|error| failed(&error))?
        .flatten()
        .find_map(|entry| {
            partition(
                &fs::read_to_string(entry.path().join("uevent")).ok()?,
                PARTITION,
            )
        })
        .ok_or(HardwareError::Absent(PARTITION))
}

#[derive(Debug)]
pub struct Partition(File);

impl Partition {
    pub fn open() -> Result<Self, HardwareError> {
        Self::find(Path::new(BLOCKS), Path::new(DEVICES))
    }

    pub fn find(blocks: &Path, devices: &Path) -> Result<Self, HardwareError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(devices.join(found(blocks)?))
            .map_err(|error| failed(&error))?;
        let size = file
            .seek(SeekFrom::End(0))
            .map_err(|error| failed(&error))?;
        if size != SIZE {
            return Err(HardwareError::Failed {
                device: PARTITION,
                reason: format!("{size} bytes, not {SIZE}"),
            });
        }
        Ok(Self(file))
    }

    fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> io::Result<()> {
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.read_exact(bytes)
    }

    fn written(&mut self, offset: u64, bytes: &[u8; SLOT_BYTES]) -> io::Result<[u8; SLOT_BYTES]> {
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.write_all(bytes)?;
        self.0.sync_all()?;
        uncached(&self.0)?;
        let mut back = [0; SLOT_BYTES];
        self.read_at(offset, &mut back)?;
        Ok(back)
    }
}

impl Settings for Partition {
    fn load(&mut self) -> Result<[u8; RECORD_BYTES], HardwareError> {
        let mut record = [0; RECORD_BYTES];
        self.read_at(0, &mut record)
            .map_err(|error| failed(&error))?;
        Ok(record)
    }

    fn save(&mut self, slot: Slot, bytes: &[u8; SLOT_BYTES]) -> Result<(), HardwareError> {
        let offset = u64::try_from(slot.offset()).map_err(|error| HardwareError::Failed {
            device: PARTITION,
            reason: error.to_string(),
        })?;
        let back = self
            .written(offset, bytes)
            .map_err(|error| failed(&error))?;
        if back == *bytes {
            Ok(())
        } else {
            Err(HardwareError::Failed {
                device: PARTITION,
                reason: format!("{slot:?} read back other than written"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use alarm_core::alarm::{Alarm, Weekdays};
    use alarm_core::settings::{Choices, SetAlarm, load, save};
    use alarm_core::sound::Volume;
    use alarm_core::time::{Countdown, TimeOfDay, UtcOffset};

    use super::*;

    const RESERVED4: &str = "DEVNAME=mmcblk0p14\nDEVTYPE=partition\nPARTN=14\nPARTNAME=reserved4\n";
    const ODM: &str = "DEVNAME=mmcblk0p44\nDEVTYPE=partition\nPARTN=44\nPARTNAME=odm\n";

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> io::Result<Self> {
            let path = std::env::temp_dir().join(format!("storage-{}-{test}", std::process::id()));
            fs::create_dir_all(path.join("blocks"))?;
            fs::create_dir_all(path.join("dev"))?;
            Ok(Self(path))
        }

        fn block(&self, name: &str, uevent: &str, size: u64) -> io::Result<()> {
            let directory = self.0.join("blocks").join(name);
            fs::create_dir_all(&directory)?;
            fs::write(directory.join("uevent"), uevent)?;
            File::create(self.0.join("dev").join(name))?.set_len(size)
        }

        fn find(&self) -> Result<Partition, HardwareError> {
            Partition::find(&self.0.join("blocks"), &self.0.join("dev"))
        }

        fn device(&self, name: &str) -> io::Result<Vec<u8>> {
            fs::read(self.0.join("dev").join(name))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn choices(minute: u8) -> Result<Choices, Box<dyn std::error::Error>> {
        Ok(Choices {
            offset: UtcOffset::from_seconds(3_600)?,
            window_seconds: 120,
            power_off: true,
            alarms: vec![SetAlarm {
                alarm: Alarm {
                    time: TimeOfDay::new(6, minute)?,
                    repeat: Weekdays::EVERY_DAY,
                },
                armed: true,
            }],
            timers: vec![Countdown::new(3, 0)?],
            lit_seconds: 30,
            volume: Volume::LOUDEST,
        })
    }

    #[test]
    fn reserved4_is_found_by_its_name_and_a_blank_one_loads_as_never_written()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("blank")?;
        scratch.block("mmcblk0p44", ODM, 4_096)?;
        scratch.block("mmcblk0p14", RESERVED4, SIZE)?;
        let mut partition = scratch.find()?;
        assert_eq!(load(&mut partition)?, None);
        Ok(())
    }

    #[test]
    fn saves_land_in_the_two_slots_and_nothing_past_them() -> Result<(), Box<dyn std::error::Error>>
    {
        let scratch = Scratch::new("saves")?;
        scratch.block("mmcblk0p14", RESERVED4, SIZE)?;
        let mut partition = scratch.find()?;
        save(&mut partition, &choices(1)?)?;
        let first = scratch.device("mmcblk0p14")?;
        assert_eq!(&first[..8], b"P20ALRM\0");
        assert!(first[SLOT_BYTES..].iter().all(|&byte| byte == 0));
        save(&mut partition, &choices(2)?)?;
        let both = scratch.device("mmcblk0p14")?;
        assert_eq!(both[..SLOT_BYTES], first[..SLOT_BYTES]);
        assert_eq!(&both[SLOT_BYTES..SLOT_BYTES + 8], b"P20ALRM\0");
        assert!(both[RECORD_BYTES..].iter().all(|&byte| byte == 0));
        assert_eq!(u64::try_from(both.len())?, SIZE);
        let mut reopened = scratch.find()?;
        assert_eq!(load(&mut reopened)?, Some(choices(2)?));
        Ok(())
    }

    #[test]
    fn a_partition_of_another_size_is_never_written() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("size")?;
        scratch.block("mmcblk0p14", RESERVED4, SIZE * 2)?;
        assert!(matches!(
            scratch.find(),
            Err(HardwareError::Failed {
                device: PARTITION,
                ..
            })
        ));
        assert!(scratch.device("mmcblk0p14")?.iter().all(|&byte| byte == 0));
        Ok(())
    }

    #[test]
    fn no_partition_named_reserved4_is_absent() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("absent")?;
        scratch.block("mmcblk0p44", ODM, 4_096)?;
        assert_eq!(scratch.find().err(), Some(HardwareError::Absent(PARTITION)));
        let missing = Partition::find(&scratch.0.join("nowhere"), &scratch.0.join("dev"));
        assert_eq!(missing.err(), Some(HardwareError::Absent(PARTITION)));
        Ok(())
    }

    #[test]
    fn a_named_partition_with_no_device_node_is_absent() -> Result<(), Box<dyn std::error::Error>> {
        let scratch = Scratch::new("node")?;
        scratch.block("mmcblk0p14", RESERVED4, SIZE)?;
        fs::remove_file(scratch.0.join("dev").join("mmcblk0p14"))?;
        assert_eq!(scratch.find().err(), Some(HardwareError::Absent(PARTITION)));
        Ok(())
    }
}
