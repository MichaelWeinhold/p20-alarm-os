use alarm_core::battery::ChargeStep;
use alarm_core::hardware::HardwareError;

use pid1::kmsg::{self, PREFIX};

pub const ENABLE_CHARGER: &str = "/sys/class/hw_power/charger/charge_data/enable_charger";
pub const ENABLE_HIZ: &str = "/sys/class/hw_power/charger/charge_data/enable_hiz";
pub const BACKLIGHT: &str = "/sys/class/leds/lcd_backlight0/brightness";
pub const INTERFACES: &str = "/sys/class/net";

pub const READINGS: [(&str, &str); 16] = [
    ("capacity", "/sys/class/power_supply/Battery/capacity"),
    ("status", "/sys/class/power_supply/Battery/status"),
    ("current_now", "/sys/class/power_supply/Battery/current_now"),
    ("voltage_now", "/sys/class/power_supply/Battery/voltage_now"),
    ("temp", "/sys/class/power_supply/Battery/temp"),
    ("health", "/sys/class/power_supply/Battery/health"),
    ("usb_online", "/sys/class/power_supply/USB/online"),
    ("ibus", "/sys/class/hw_power/charger/charge_data/Ibus"),
    ("vbus", "/sys/class/hw_power/charger/charge_data/Vbus"),
    ("enable_charger", ENABLE_CHARGER),
    ("enable_hiz", ENABLE_HIZ),
    ("backlight", BACKLIGHT),
    ("modem", "/sys/devices/platform/balong_power/modem_state"),
    ("cpus", "/sys/devices/system/cpu/online"),
    (
        "freq0",
        "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
    ),
    (
        "freq4",
        "/sys/devices/system/cpu/cpu4/cpufreq/scaling_cur_freq",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Sample(&'static str),
    Write(&'static str, &'static str),
    StopCharge,
    Wait(u64),
}

const PLUG_IN: Step = Step::Write(ENABLE_CHARGER, "1");

fn settled(steps: &mut Vec<Step>, phase: &'static str) {
    steps.push(Step::Wait(5));
    steps.push(Step::Sample(phase));
}

#[must_use]
pub fn plan() -> Vec<Step> {
    let mut steps = vec![Step::Sample("start"), Step::StopCharge];
    settled(&mut steps, "stopped");
    steps.push(PLUG_IN);
    settled(&mut steps, "plugged_in");
    steps.push(Step::StopCharge);
    settled(&mut steps, "stopped_again");
    steps.push(PLUG_IN);
    settled(&mut steps, "restored");
    steps
}

#[must_use]
pub fn stop(result: &Result<ChargeStep, HardwareError>) -> String {
    match result {
        Ok(step) => format!("{PREFIX}charge stop: {step:?}\n"),
        Err(error) => kmsg::failure("charge stop", error),
    }
}

fn token(value: &str) -> String {
    let words: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        "-".to_owned()
    } else {
        words.join("_")
    }
}

#[must_use]
pub fn line(phase: &str, readings: &[(&str, Result<String, String>)]) -> String {
    let fields: Vec<String> = readings
        .iter()
        .map(|(name, reading)| match reading {
            Ok(value) => format!("{name}={}", token(value)),
            Err(error) => format!("{name}=ERR({})", token(error)),
        })
        .collect();
    format!("{PREFIX}power {phase}: {}\n", fields.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_stops_twice_around_a_plug_in_and_leaves_the_charger_on() {
        let steps = plan();
        let positions: Vec<usize> = steps
            .iter()
            .enumerate()
            .filter(|(_, step)| **step == Step::StopCharge)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(positions.len(), 2);
        assert!(steps[positions[0]..positions[1]].contains(&PLUG_IN));
        let last_write = steps
            .iter()
            .rev()
            .find(|step| matches!(step, Step::Write(..)));
        assert_eq!(last_write, Some(&PLUG_IN));
        assert!(
            !steps
                .iter()
                .any(|step| matches!(step, Step::Write(path, _) if *path != ENABLE_CHARGER))
        );
    }

    #[test]
    fn the_plan_samples_after_every_stop_and_write() {
        let steps = plan();
        assert_eq!(steps.first(), Some(&Step::Sample("start")));
        assert_eq!(steps.last(), Some(&Step::Sample("restored")));
        for (index, step) in steps.iter().enumerate() {
            if matches!(step, Step::StopCharge | Step::Write(..)) {
                assert_eq!(steps.get(index + 1), Some(&Step::Wait(5)), "{index}");
                assert!(
                    matches!(steps.get(index + 2), Some(Step::Sample(_))),
                    "{index}"
                );
            }
        }
    }

    #[test]
    fn the_plan_finishes_well_inside_the_boot_timers_ten_minutes() {
        let waited: u64 = plan()
            .iter()
            .map(|step| match step {
                Step::Wait(seconds) => *seconds,
                _ => 0,
            })
            .sum();
        assert!(waited < 420, "{waited} s");
    }

    #[test]
    fn a_stop_logs_its_step_or_its_failure() {
        assert_eq!(
            stop(&Ok(ChargeStep::Stopped)),
            "<3>p20-alarm-os: charge stop: Stopped\n"
        );
        assert_eq!(
            stop(&Err(HardwareError::Absent("Battery/capacity"))),
            "<3>p20-alarm-os: charge stop failed: Battery/capacity is not present on this device\n"
        );
    }

    #[test]
    fn a_reading_is_one_token_and_a_failure_is_marked() {
        let readings = [
            ("status", Ok("Not charging\n".to_owned())),
            (
                "modem",
                Err("No such file or directory (os error 2)".to_owned()),
            ),
            ("backlight", Ok(String::new())),
        ];
        assert_eq!(
            line("start", &readings),
            "<3>p20-alarm-os: power start: status=Not_charging \
             modem=ERR(No_such_file_or_directory_(os_error_2)) backlight=-\n"
        );
    }

    #[test]
    fn a_nul_in_a_reading_never_reaches_the_kernel_log() {
        let readings = [
            ("modem", Ok("MODEM_STATE_OFF\n\0".to_owned())),
            ("cpus", Ok("0-7\n".to_owned())),
        ];
        assert_eq!(
            line("start", &readings),
            "<3>p20-alarm-os: power start: modem=MODEM_STATE_OFF cpus=0-7\n"
        );
    }

    #[test]
    fn a_line_with_every_reading_failing_still_fits_one_kmsg_record() {
        let error = "Permission denied (os error 13)".to_owned();
        let readings: Vec<(&str, Result<String, String>)> = READINGS
            .iter()
            .map(|(name, _)| (*name, Err(error.clone())))
            .collect();
        let text = line("stopped_again", &readings);
        assert!(text.len() < 992, "{} bytes", text.len());
        assert_eq!(text.matches('\n').count(), 1);
    }
}
