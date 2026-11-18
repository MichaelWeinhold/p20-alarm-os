pub const BLOCKS: &str = "/sys/class/block";

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
    use super::*;

    const ODM: &str = "DEVNAME=mmcblk0p44\nDEVTYPE=partition\nPARTN=44\nPARTNAME=odm\n";

    #[test]
    fn the_partition_is_found_by_its_name() {
        assert_eq!(partition(ODM, "odm").as_deref(), Some("mmcblk0p44"));
        assert_eq!(partition(ODM, "system"), None);
        assert_eq!(partition("PARTNAME=odm\n", "odm"), None);
    }
}
