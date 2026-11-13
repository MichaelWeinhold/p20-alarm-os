use std::path::PathBuf;

pub const REQUESTS: &str = "/sys/class/firmware";
pub const BLOCKS: &str = "/sys/class/block";
pub const PARTITION: &str = "odm";
pub const DIRECTORY: &str = "etc/firmware";

#[must_use]
pub fn requested(entry: &str) -> Option<PathBuf> {
    if entry == "timeout" || entry.split('!').any(|part| matches!(part, "" | "." | "..")) {
        return None;
    }
    Some(entry.split('!').collect())
}

#[must_use]
pub fn partition(uevent: &str, name: &str) -> Option<String> {
    let mut device = None;
    let mut named = false;
    for line in uevent.lines() {
        match line.split_once('=') {
            Some(("DEVNAME", value)) => device = Some(value),
            Some(("PARTNAME", value)) => named = value == name,
            _ => {}
        }
    }
    device.filter(|_| named).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const ODM: &str = "DEVNAME=mmcblk0p44\nDEVTYPE=partition\nPARTN=44\nPARTNAME=odm\n";

    #[test]
    fn a_request_names_a_file_under_the_firmware_directory() {
        assert_eq!(
            requested("ts!anne_gt1x_1158_auo.bin").as_deref(),
            Some(Path::new("ts/anne_gt1x_1158_auo.bin"))
        );
        assert_eq!(
            requested("anne.img").as_deref(),
            Some(Path::new("anne.img"))
        );
    }

    #[test]
    fn the_timeout_and_a_path_out_of_the_directory_are_no_request() {
        assert_eq!(requested("timeout"), None);
        assert_eq!(requested(""), None);
        assert_eq!(requested("ts!..!..!..!etc!shadow"), None);
        assert_eq!(requested("!etc!shadow"), None);
        assert_eq!(requested("ts!.!x"), None);
    }

    #[test]
    fn the_partition_is_found_by_its_name() {
        assert_eq!(partition(ODM, "odm").as_deref(), Some("mmcblk0p44"));
        assert_eq!(partition(ODM, "system"), None);
        assert_eq!(partition("PARTNAME=odm\n", "odm"), None);
    }
}
