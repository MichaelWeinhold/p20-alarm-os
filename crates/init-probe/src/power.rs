use crate::marker::PREFIX;

pub const ENABLE_CHARGER: &str = "/sys/class/hw_power/charger/charge_data/enable_charger";
pub const ENABLE_HIZ: &str = "/sys/class/hw_power/charger/charge_data/enable_hiz";
pub const BACKLIGHT: &str = "/sys/class/leds/lcd_backlight0/brightness";
pub const FB_BLANK: &str = "/sys/class/graphics/fb0/blank";
pub const INTERFACES: &str = "/sys/class/net";

const GOVERNORS: [&str; 2] = [
    "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
    "/sys/devices/system/cpu/cpu4/cpufreq/scaling_governor",
];

const SECONDARY_CPUS: [&str; 7] = [
    "/sys/devices/system/cpu/cpu1/online",
    "/sys/devices/system/cpu/cpu2/online",
    "/sys/devices/system/cpu/cpu3/online",
    "/sys/devices/system/cpu/cpu4/online",
    "/sys/devices/system/cpu/cpu5/online",
    "/sys/devices/system/cpu/cpu6/online",
    "/sys/devices/system/cpu/cpu7/online",
];

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
    Wait(u64),
}

fn sampled(steps: &mut Vec<Step>, phase: &'static str, count: usize, every: u64) {
    for _ in 0..count {
        steps.push(Step::Wait(every));
        steps.push(Step::Sample(phase));
    }
}

#[must_use]
pub fn plan() -> Vec<Step> {
    let mut steps = vec![Step::Sample("start"), Step::Write(ENABLE_HIZ, "1")];
    sampled(&mut steps, "hiz_defaults", 5, 4);
    for governor in GOVERNORS {
        steps.push(Step::Write(governor, "powersave"));
    }
    sampled(&mut steps, "hiz_powersave", 5, 4);
    for cpu in SECONDARY_CPUS {
        steps.push(Step::Write(cpu, "0"));
    }
    sampled(&mut steps, "hiz_cpu0_only", 5, 4);
    steps.push(Step::Write(FB_BLANK, "4"));
    sampled(&mut steps, "hiz_fb_powerdown", 5, 4);
    for cpu in SECONDARY_CPUS {
        steps.push(Step::Write(cpu, "1"));
    }
    sampled(&mut steps, "hiz_fb_powerdown_all_cpus", 5, 4);
    steps.push(Step::Write(ENABLE_HIZ, "0"));
    sampled(&mut steps, "restored", 1, 5);
    steps
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
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn the_plan_leaves_the_input_on_and_every_cpu_online() {
        let mut last: HashMap<&str, &str> = HashMap::new();
        for step in plan() {
            if let Step::Write(path, value) = step {
                last.insert(path, value);
            }
        }
        assert_eq!(last.get(ENABLE_HIZ), Some(&"0"));
        for cpu in SECONDARY_CPUS {
            assert_eq!(last.get(cpu), Some(&"1"), "{cpu}");
        }
        assert!(!last.contains_key(ENABLE_CHARGER));
    }

    #[test]
    fn the_plan_samples_before_and_after_every_write() {
        let steps = plan();
        assert_eq!(steps.first(), Some(&Step::Sample("start")));
        assert_eq!(steps.last(), Some(&Step::Sample("restored")));
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
        let text = line("hiz_fb_powerdown_all_cpus", &readings);
        assert!(text.len() < 992, "{} bytes", text.len());
        assert_eq!(text.matches('\n').count(), 1);
    }
}
