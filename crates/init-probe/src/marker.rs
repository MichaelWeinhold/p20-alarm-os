use std::fmt::Display;

pub(crate) const PREFIX: &str = "<3>p20-alarm-os: ";
const CHUNK: usize = 400;

pub const MARKER: &str = "/init ran from system";

#[must_use]
pub fn marker() -> String {
    format!("{PREFIX}{MARKER}\n")
}

#[must_use]
pub fn cmdline(text: &str) -> Vec<String> {
    let chunks: Vec<&[u8]> = text.trim_end().as_bytes().chunks(CHUNK).collect();
    let count = chunks.len();
    chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            format!(
                "{PREFIX}cmdline {}/{count}: {}\n",
                index + 1,
                String::from_utf8_lossy(chunk)
            )
        })
        .collect()
}

#[must_use]
pub fn failure(step: &str, error: &impl Display) -> String {
    format!("{PREFIX}{step} failed: {error}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn priority(line: &str) -> Option<u8> {
        line.strip_prefix('<')?.split_once('>')?.0.parse().ok()
    }

    #[test]
    fn the_marker_is_one_line_below_the_command_lines_loglevel() {
        let line = marker();
        assert!(priority(&line).is_some_and(|level| level < 4));
        assert!(line.ends_with(&format!("{MARKER}\n")));
        assert_eq!(line.matches('\n').count(), 1);
    }

    #[test]
    fn a_long_cmdline_splits_into_numbered_lines_that_join_back() {
        let text = format!("{}\n", "loglevel=4 ".repeat(100));
        let lines = cmdline(&text);
        assert_eq!(lines.len(), 3);
        let mut joined = String::new();
        for (index, line) in lines.iter().enumerate() {
            assert!(priority(line).is_some_and(|level| level < 4));
            assert!(line.len() < 992);
            let label = format!("cmdline {}/3: ", index + 1);
            let (_, rest) = line.split_once(&label).unwrap_or_default();
            joined.push_str(rest.trim_end_matches('\n'));
        }
        assert_eq!(joined, text.trim_end());
    }

    #[test]
    fn a_short_cmdline_is_one_line() {
        assert_eq!(
            cmdline("console=ttyS0\n"),
            vec!["<3>p20-alarm-os: cmdline 1/1: console=ttyS0\n".to_owned()]
        );
    }

    #[test]
    fn a_failure_names_its_step_and_error() {
        assert_eq!(
            failure("mounting /proc", &"No such device"),
            "<3>p20-alarm-os: mounting /proc failed: No such device\n"
        );
    }
}
