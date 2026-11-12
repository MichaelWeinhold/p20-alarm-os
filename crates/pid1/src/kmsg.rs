use std::fmt::Display;

pub const PREFIX: &str = "<3>p20-alarm-os: ";

#[must_use]
pub fn line(text: &str) -> String {
    format!("{PREFIX}{text}\n")
}

#[must_use]
pub fn failure(step: &str, error: &impl Display) -> String {
    format!("{PREFIX}{step} failed: {error}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_prefixed_and_ends_once() {
        assert_eq!(
            line("debug: mounted proc on /proc"),
            "<3>p20-alarm-os: debug: mounted proc on /proc\n"
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
