use alarm_core::hardware::HardwareError;
use alarm_core::input::{InputEvent, Key};

pub mod firmware;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod linux;

pub const RECORD_SIZE: usize = 24;
pub const BUTTONS: &str = "hisi_gpio_key";
pub const POWER: &str = "hisi_on";
pub const TOUCH: &str = "huawei,ts_kit";
pub const DEVICES: [&str; 3] = [BUTTONS, POWER, TOUCH];

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const SYN_DROPPED: u16 = 3;
const KEY_VOLUMEDOWN: u16 = 114;
const KEY_VOLUMEUP: u16 = 115;
const KEY_POWER: u16 = 116;
const BTN_TOUCH: u16 = 0x14a;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const PRESSED: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

impl Record {
    #[must_use]
    pub fn parse(bytes: &[u8; RECORD_SIZE]) -> Self {
        let [.., k0, k1, c0, c1, v0, v1, v2, v3] = *bytes;
        Self {
            kind: u16::from_le_bytes([k0, k1]),
            code: u16::from_le_bytes([c0, c1]),
            value: i32::from_le_bytes([v0, v1, v2, v3]),
        }
    }
}

fn failed(device: &'static str, reason: String) -> HardwareError {
    HardwareError::Failed { device, reason }
}

pub fn records(device: &'static str, bytes: &[u8]) -> Result<Vec<Record>, HardwareError> {
    let (whole, rest) = bytes.as_chunks::<RECORD_SIZE>();
    if !rest.is_empty() {
        return Err(failed(
            device,
            format!("{} bytes follow the last whole record", rest.len()),
        ));
    }
    Ok(whole.iter().map(Record::parse).collect())
}

fn key(code: u16) -> Option<Key> {
    match code {
        KEY_VOLUMEUP => Some(Key::VolumeUp),
        KEY_VOLUMEDOWN => Some(Key::VolumeDown),
        KEY_POWER => Some(Key::Power),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decoder {
    x: Option<usize>,
    y: Option<usize>,
    touched: bool,
    dropped: bool,
}

impl Decoder {
    pub fn feed(
        &mut self,
        device: &'static str,
        record: Record,
    ) -> Result<Option<InputEvent>, HardwareError> {
        let position = || {
            usize::try_from(record.value)
                .map_err(|_| failed(device, format!("a position of {}", record.value)))
        };
        match (record.kind, record.code) {
            (EV_SYN, SYN_DROPPED) => {
                *self = Self {
                    dropped: true,
                    ..Self::default()
                };
            }
            (EV_SYN, SYN_REPORT) => {
                let frame = std::mem::take(self);
                if frame.touched && !frame.dropped {
                    return match (frame.x, frame.y) {
                        (Some(x), Some(y)) => Ok(Some(InputEvent::Touch { x, y })),
                        _ => Err(failed(device, "a touch without a position".to_owned())),
                    };
                }
            }
            _ if self.dropped => {}
            (EV_KEY, _) if record.value != PRESSED => {}
            (EV_ABS, ABS_MT_POSITION_X) => self.x = Some(self.x.map_or_else(position, Ok)?),
            (EV_ABS, ABS_MT_POSITION_Y) => self.y = Some(self.y.map_or_else(position, Ok)?),
            (EV_KEY, BTN_TOUCH) => self.touched = true,
            (EV_KEY, code) => return Ok(key(code).map(InputEvent::Press)),
            (EV_SYN | EV_ABS, _) => {}
            (kind, code) => {
                return Err(failed(
                    device,
                    format!("an event of type {kind} and code {code}"),
                ));
            }
        }
        Ok(None)
    }
}

pub fn events(
    device: &'static str,
    bytes: &[u8],
    decoder: &mut Decoder,
) -> Result<Vec<InputEvent>, HardwareError> {
    let mut events = Vec::new();
    for record in records(device, bytes)? {
        events.extend(decoder.feed(device, record)?);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAPS: &[u8] = include_bytes!("../testdata/taps");
    const VOLUME_KEYS: &[u8] = include_bytes!("../testdata/volume-keys");
    const POWER_KEY: &[u8] = include_bytes!("../testdata/power-key");

    fn record(kind: u16, code: u16, value: i32) -> [u8; RECORD_SIZE] {
        let mut bytes = [0; RECORD_SIZE];
        bytes[16..18].copy_from_slice(&kind.to_le_bytes());
        bytes[18..20].copy_from_slice(&code.to_le_bytes());
        bytes[20..].copy_from_slice(&value.to_le_bytes());
        bytes
    }

    fn joined(records: &[[u8; RECORD_SIZE]]) -> Vec<u8> {
        records.concat()
    }

    #[test]
    fn four_recorded_taps_are_four_touches_where_they_landed() -> Result<(), HardwareError> {
        let events = events(TOUCH, TAPS, &mut Decoder::default())?;
        assert_eq!(
            events,
            [
                InputEvent::Touch { x: 509, y: 1100 },
                InputEvent::Touch { x: 564, y: 1142 },
                InputEvent::Touch { x: 645, y: 1209 },
                InputEvent::Touch { x: 635, y: 1139 },
            ]
        );
        Ok(())
    }

    #[test]
    fn the_recorded_volume_keys_are_one_press_each() -> Result<(), HardwareError> {
        let events = events(BUTTONS, VOLUME_KEYS, &mut Decoder::default())?;
        assert_eq!(
            events,
            [
                InputEvent::Press(Key::VolumeUp),
                InputEvent::Press(Key::VolumeDown),
                InputEvent::Press(Key::VolumeDown),
            ]
        );
        Ok(())
    }

    #[test]
    fn the_recorded_power_key_is_one_press() -> Result<(), HardwareError> {
        let events = events(POWER, POWER_KEY, &mut Decoder::default())?;
        assert_eq!(events, [InputEvent::Press(Key::Power)]);
        Ok(())
    }

    #[test]
    fn a_truncated_record_is_refused() {
        let truncated = &TAPS[..TAPS.len() - 1];
        assert!(matches!(
            events(TOUCH, truncated, &mut Decoder::default()),
            Err(HardwareError::Failed { device: TOUCH, .. })
        ));
    }

    #[test]
    fn an_event_type_it_does_not_expect_is_refused() {
        let bytes = joined(&[record(4, 4, 7)]);
        assert!(events(BUTTONS, &bytes, &mut Decoder::default()).is_err());
    }

    #[test]
    fn a_touch_without_a_position_is_refused() {
        let bytes = joined(&[record(EV_KEY, BTN_TOUCH, 1), record(EV_SYN, SYN_REPORT, 0)]);
        assert!(events(TOUCH, &bytes, &mut Decoder::default()).is_err());
    }

    #[test]
    fn a_negative_position_is_refused() {
        let bytes = joined(&[record(EV_ABS, ABS_MT_POSITION_X, -1)]);
        assert!(events(TOUCH, &bytes, &mut Decoder::default()).is_err());
    }

    #[test]
    fn records_after_a_drop_are_skipped_until_the_next_report() -> Result<(), HardwareError> {
        let bytes = joined(&[
            record(EV_ABS, ABS_MT_POSITION_X, 10),
            record(EV_SYN, SYN_DROPPED, 0),
            record(EV_ABS, ABS_MT_POSITION_Y, 20),
            record(EV_KEY, BTN_TOUCH, 1),
            record(EV_KEY, KEY_POWER, 1),
            record(EV_SYN, SYN_REPORT, 0),
            record(EV_KEY, KEY_VOLUMEUP, 1),
            record(EV_SYN, SYN_REPORT, 0),
        ]);
        let events = events(BUTTONS, &bytes, &mut Decoder::default())?;
        assert_eq!(events, [InputEvent::Press(Key::VolumeUp)]);
        Ok(())
    }

    #[test]
    fn a_held_touch_and_a_released_key_add_nothing() -> Result<(), HardwareError> {
        let bytes = joined(&[
            record(EV_ABS, ABS_MT_POSITION_X, 10),
            record(EV_ABS, ABS_MT_POSITION_Y, 20),
            record(EV_SYN, SYN_REPORT, 0),
            record(EV_KEY, KEY_POWER, 0),
            record(EV_KEY, 0x2ff, 1),
            record(EV_SYN, SYN_REPORT, 0),
        ]);
        assert_eq!(events(TOUCH, &bytes, &mut Decoder::default())?, []);
        Ok(())
    }
}
