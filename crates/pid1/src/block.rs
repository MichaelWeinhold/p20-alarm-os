pub const BLOCKS: &str = "/sys/class/block";

#[must_use]
pub fn named_device(uevent: &str) -> Option<(String, String)> {
    let mut device = None;
    let mut name = None;
    for line in uevent.lines() {
        match line.split_once('=') {
            Some(("DEVNAME", value)) => device = Some(value),
            Some(("PARTNAME", value)) => name = Some(value),
            _ => {}
        }
    }
    Some((name?.to_owned(), device?.to_owned()))
}

#[must_use]
pub fn partition(uevent: &str, name: &str) -> Option<String> {
    named_device(uevent)
        .filter(|(named, _)| named == name)
        .map(|(_, device)| device)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ODM: &str = "DEVNAME=mmcblk0p44\nDEVTYPE=partition\nPARTN=44\nPARTNAME=odm\n";

    #[test]
    fn the_partition_is_found_by_its_name() {
        assert_eq!(partition(ODM, "odm").as_deref(), Some("mmcblk0p44"));
        assert_eq!(partition(ODM, "system"), None);
        assert_eq!(partition("PARTNAME=odm\n", "odm"), None);
    }

    #[test]
    fn a_partition_names_itself_and_its_device() {
        assert_eq!(
            named_device(ODM),
            Some(("odm".to_owned(), "mmcblk0p44".to_owned()))
        );
        assert_eq!(named_device("DEVNAME=mmcblk0\nDEVTYPE=disk\n"), None);
    }
}
