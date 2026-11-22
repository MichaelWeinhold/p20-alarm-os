use alarm_core::hardware::HardwareError;

use crate::failed;

pub const SKIPPED: &str = "CLASSD_VOLTAGE_CONFIG";

const DEVICE: &str = "speaker-route.tsv";
const DEFAULT: &str = "default";
const SPEAKER: &str = "speaker";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub ring: Vec<Control>,
    pub after: Vec<Control>,
}

fn line(number: usize, text: &str) -> Result<(bool, Control), HardwareError> {
    let unexpected = || failed(DEVICE, format!("line {number} is {text:?}"));
    let mut fields = text.split('\t');
    let (Some(set), Some(name), Some(value), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(unexpected());
    };
    if [name, value]
        .iter()
        .any(|field| field.is_empty() || field.chars().any(char::is_control))
    {
        return Err(unexpected());
    }
    let speaker = match set {
        DEFAULT => false,
        SPEAKER => true,
        _ => return Err(unexpected()),
    };
    Ok((
        speaker,
        Control {
            name: name.to_owned(),
            value: value.to_owned(),
        },
    ))
}

pub fn parse(text: &str) -> Result<Route, HardwareError> {
    let body = text
        .strip_suffix('\n')
        .ok_or_else(|| failed(DEVICE, "no final newline".to_owned()))?;
    let mut defaults = Vec::new();
    let mut speaker = Vec::new();
    for (index, text) in body.split('\n').enumerate() {
        let (in_speaker, control) = line(index + 1, text)?;
        if in_speaker {
            speaker.push(control);
        } else if speaker.is_empty() {
            defaults.push(control);
        } else {
            return Err(failed(
                DEVICE,
                format!("a default on line {} after the speaker's", index + 1),
            ));
        }
    }
    if defaults.is_empty() || speaker.is_empty() {
        return Err(failed(
            DEVICE,
            format!(
                "{} defaults and {} speaker controls",
                defaults.len(),
                speaker.len()
            ),
        ));
    }
    let after = speaker
        .iter()
        .filter(|control| control.name != SKIPPED)
        .map(|control| {
            defaults
                .iter()
                .rev()
                .find(|default| default.name == control.name)
                .cloned()
                .ok_or_else(|| failed(DEVICE, format!("no default for {}", control.name)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ring = defaults
        .into_iter()
        .chain(speaker)
        .filter(|control| control.name != SKIPPED)
        .collect();
    Ok(Route { ring, after })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(name: &str, value: &str) -> Control {
        Control {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    const RECORDED: &str = "default\tDACL PGA\t0\n\
        default\tSTEREO_DLINK_TEST_SEL MUX\tOff\n\
        default\tCLASSD_VOLTAGE_CONFIG\t5200\n\
        default\tCLASSD VIR_SWITCH CLASSD_EN\t0\n\
        speaker\tSTEREO_DLINK_TEST_SEL MUX\tAPB\n\
        speaker\tCLASSD VIR_SWITCH CLASSD_EN\t1\n";

    #[test]
    fn a_ring_sets_the_defaults_then_the_speaker_less_the_voltage() -> Result<(), HardwareError> {
        let route = parse(RECORDED)?;
        assert_eq!(
            route.ring,
            [
                control("DACL PGA", "0"),
                control("STEREO_DLINK_TEST_SEL MUX", "Off"),
                control("CLASSD VIR_SWITCH CLASSD_EN", "0"),
                control("STEREO_DLINK_TEST_SEL MUX", "APB"),
                control("CLASSD VIR_SWITCH CLASSD_EN", "1"),
            ]
        );
        Ok(())
    }

    #[test]
    fn after_a_ring_the_speaker_controls_go_back_to_their_defaults() -> Result<(), HardwareError> {
        let route = parse(RECORDED)?;
        assert_eq!(
            route.after,
            [
                control("STEREO_DLINK_TEST_SEL MUX", "Off"),
                control("CLASSD VIR_SWITCH CLASSD_EN", "0"),
            ]
        );
        Ok(())
    }

    #[test]
    fn the_last_default_of_a_name_is_the_one_restored() -> Result<(), HardwareError> {
        let route = parse("default\tA\t1\ndefault\tA\t2\nspeaker\tA\t3\n")?;
        assert_eq!(route.after, [control("A", "2")]);
        Ok(())
    }

    #[test]
    fn a_malformed_route_is_a_hardware_error() {
        for text in [
            "",
            "\n",
            "default\tA\t1\nspeaker\tA\t1",
            "default\tA\t1\n",
            "speaker\tA\t1\n",
            "default\tA\t1\nspeaker\tB\t1\n",
            "default\tA\t1\nspeaker\tA\t1\ndefault\tA\t0\n",
            "default\tA\nspeaker\tA\t1\n",
            "default\tA\t1\t2\nspeaker\tA\t1\n",
            "default\t\t1\nspeaker\tA\t1\n",
            "default\tA\t\nspeaker\tA\t1\n",
            "other\tA\t1\nspeaker\tA\t1\n",
            "default\tA\t1\r\nspeaker\tA\t1\n",
        ] {
            assert!(
                matches!(
                    parse(text),
                    Err(HardwareError::Failed { device: DEVICE, .. })
                ),
                "{text:?}"
            );
        }
    }
}
