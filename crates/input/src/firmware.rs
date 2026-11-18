use std::path::PathBuf;

pub const REQUESTS: &str = "/sys/class/firmware";
pub const PARTITION: &str = "odm";
pub const DIRECTORY: &str = "etc/firmware";

#[must_use]
pub fn requested(entry: &str) -> Option<PathBuf> {
    if entry == "timeout" || entry.split('!').any(|part| matches!(part, "" | "." | "..")) {
        return None;
    }
    Some(entry.split('!').collect())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

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
}
