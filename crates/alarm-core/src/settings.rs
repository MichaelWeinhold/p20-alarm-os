use crate::alarm::{Alarm, Weekdays};
use crate::hardware::{HardwareError, Settings};
use crate::time::{TimeOfDay, UtcOffset};

pub const RECORD_BYTES: usize = 2 * SLOT_BYTES;
pub const SLOT_BYTES: usize = 4096;
pub const MAGIC: [u8; 8] = *b"P20ALRM\0";
pub const VERSION: u32 = 2;
pub const FIRST_VERSION: u32 = 1;
pub const LARGEST_PAYLOAD: usize = SLOT_BYTES - HEADER_BYTES - CRC_BYTES;

const HEADER_BYTES: usize = 24;
const CRC_BYTES: usize = 4;
const CHOICES_BYTES: usize = 10;
const FIRST_CHOICES_BYTES: usize = 9;
const ALARM_BYTES: usize = 4;
const DEVICE: &str = "settings";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    First,
    Second,
}

impl Slot {
    #[must_use]
    pub fn offset(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => SLOT_BYTES,
        }
    }

    fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SetAlarm {
    pub alarm: Alarm,
    pub armed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Choices {
    pub offset: UtcOffset,
    pub window_seconds: i64,
    pub power_off: bool,
    pub alarms: Vec<SetAlarm>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Valid {
    sequence: u64,
    choices: Choices,
}

#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn refused(reason: String) -> HardwareError {
    HardwareError::Failed {
        device: DEVICE,
        reason,
    }
}

fn payload(choices: &Choices) -> Result<Vec<u8>, HardwareError> {
    let offset = i32::try_from(choices.offset.seconds())
        .map_err(|_| refused(format!("an offset of {}", choices.offset.seconds())))?;
    let window = u32::try_from(choices.window_seconds)
        .map_err(|_| refused(format!("a window of {} s", choices.window_seconds)))?;
    let count = u8::try_from(choices.alarms.len())
        .map_err(|_| refused(format!("{} alarms", choices.alarms.len())))?;
    let mut bytes = Vec::with_capacity(CHOICES_BYTES + ALARM_BYTES * choices.alarms.len());
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(&window.to_le_bytes());
    bytes.push(u8::from(choices.power_off));
    bytes.push(count);
    for set in &choices.alarms {
        bytes.extend_from_slice(&[
            set.alarm.time.hour(),
            set.alarm.time.minute(),
            set.alarm.repeat.bits(),
            u8::from(set.armed),
        ]);
    }
    Ok(bytes)
}

pub fn framed(payload: &[u8], sequence: u64) -> Result<[u8; SLOT_BYTES], HardwareError> {
    framed_as(VERSION, payload, sequence)
}

fn framed_as(
    version: u32,
    payload: &[u8],
    sequence: u64,
) -> Result<[u8; SLOT_BYTES], HardwareError> {
    let length = u32::try_from(payload.len())
        .ok()
        .filter(|_| payload.len() <= LARGEST_PAYLOAD)
        .ok_or_else(|| refused(format!("a payload of {} bytes", payload.len())))?;
    let end = HEADER_BYTES + payload.len();
    let mut slot = [0; SLOT_BYTES];
    slot[..8].copy_from_slice(&MAGIC);
    slot[8..12].copy_from_slice(&version.to_le_bytes());
    slot[12..16].copy_from_slice(&length.to_le_bytes());
    slot[16..HEADER_BYTES].copy_from_slice(&sequence.to_le_bytes());
    slot[HEADER_BYTES..end].copy_from_slice(payload);
    let crc = crc32(&slot[..end]);
    slot[end..end + CRC_BYTES].copy_from_slice(&crc.to_le_bytes());
    Ok(slot)
}

pub fn encode(choices: &Choices, sequence: u64) -> Result<[u8; SLOT_BYTES], HardwareError> {
    framed(&payload(choices)?, sequence)
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)?
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
}

fn flag(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn set_alarm([hour, minute, repeat, armed]: [u8; ALARM_BYTES]) -> Option<SetAlarm> {
    let armed = flag(armed)?;
    Some(SetAlarm {
        alarm: Alarm {
            time: TimeOfDay::new(hour, minute).ok()?,
            repeat: Weekdays::from_bits(repeat)?,
        },
        armed,
    })
}

fn choices(payload: &[u8], version: u32) -> Option<Choices> {
    let offset = UtcOffset::from_seconds(word(payload, 0)?.cast_signed()).ok()?;
    let window = word(payload, 4)?;
    let (power_off, fixed) = if version == FIRST_VERSION {
        (true, FIRST_CHOICES_BYTES)
    } else {
        (flag(*payload.get(8)?)?, CHOICES_BYTES)
    };
    let count = usize::from(*payload.get(fixed - 1)?);
    let (entries, rest) = payload.get(fixed..)?.as_chunks::<ALARM_BYTES>();
    if entries.len() != count || !rest.is_empty() {
        return None;
    }
    let alarms = entries
        .iter()
        .copied()
        .map(set_alarm)
        .collect::<Option<Vec<_>>>()?;
    Some(Choices {
        offset,
        window_seconds: i64::from(window),
        power_off,
        alarms,
    })
}

fn decode(slot: &[u8]) -> Option<Valid> {
    let version = word(slot, 8)?;
    if slot.get(..8)? != MAGIC || !(FIRST_VERSION..=VERSION).contains(&version) {
        return None;
    }
    let length = usize::try_from(word(slot, 12)?).ok()?;
    if length > LARGEST_PAYLOAD {
        return None;
    }
    let sequence = u64::from_le_bytes(slot.get(16..HEADER_BYTES)?.try_into().ok()?);
    let end = HEADER_BYTES + length;
    if crc32(slot.get(..end)?) != word(slot, end)? {
        return None;
    }
    let choices = choices(slot.get(HEADER_BYTES..end)?, version)?;
    Some(Valid { sequence, choices })
}

fn newer(sequence: u64, than: u64) -> bool {
    sequence.wrapping_sub(than).cast_signed() > 0
}

fn newest(record: &[u8; RECORD_BYTES]) -> Option<(Slot, Valid)> {
    let (first, second) = record.split_at(SLOT_BYTES);
    match (decode(first), decode(second)) {
        (Some(first), Some(second)) if newer(second.sequence, first.sequence) => {
            Some((Slot::Second, second))
        }
        (Some(first), _) => Some((Slot::First, first)),
        (None, Some(second)) => Some((Slot::Second, second)),
        (None, None) => None,
    }
}

pub fn load(settings: &mut impl Settings) -> Result<Option<Choices>, HardwareError> {
    let record = settings.load()?;
    if let Some((_, valid)) = newest(&record) {
        return Ok(Some(valid.choices));
    }
    if record.iter().all(|&byte| byte == 0) {
        return Ok(None);
    }
    Err(refused(
        "no valid slot, and the record is not blank".to_owned(),
    ))
}

pub fn save(settings: &mut impl Settings, choices: &Choices) -> Result<(), HardwareError> {
    let record = settings.load()?;
    let (slot, sequence) = newest(&record).map_or((Slot::First, 0), |(slot, valid)| {
        (slot.other(), valid.sequence.wrapping_add(1))
    });
    settings.save(slot, &encode(choices, sequence)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TimeError;

    struct FakeRecord {
        bytes: [u8; RECORD_BYTES],
        saved: Vec<Slot>,
        broken: bool,
    }

    impl FakeRecord {
        fn blank() -> Self {
            Self {
                bytes: [0; RECORD_BYTES],
                saved: Vec::new(),
                broken: false,
            }
        }

        fn put(&mut self, slot: Slot, bytes: &[u8; SLOT_BYTES]) {
            self.bytes[slot.offset()..slot.offset() + SLOT_BYTES].copy_from_slice(bytes);
        }

        fn sequence(&self, slot: Slot) -> Option<u64> {
            decode(&self.bytes[slot.offset()..slot.offset() + SLOT_BYTES])
                .map(|valid| valid.sequence)
        }
    }

    impl Settings for FakeRecord {
        fn load(&mut self) -> Result<[u8; RECORD_BYTES], HardwareError> {
            if self.broken {
                return Err(HardwareError::Absent("reserved4"));
            }
            Ok(self.bytes)
        }

        fn save(&mut self, slot: Slot, bytes: &[u8; SLOT_BYTES]) -> Result<(), HardwareError> {
            self.put(slot, bytes);
            self.saved.push(slot);
            Ok(())
        }
    }

    fn alarm_at(hour: u8, minute: u8, armed: bool) -> Result<SetAlarm, TimeError> {
        Ok(SetAlarm {
            alarm: Alarm {
                time: TimeOfDay::new(hour, minute)?,
                repeat: Weekdays::EVERY_DAY,
            },
            armed,
        })
    }

    fn choices_at(hour: u8, minute: u8) -> Result<Choices, TimeError> {
        Ok(Choices {
            offset: UtcOffset::from_seconds(3_600)?,
            window_seconds: 120,
            power_off: true,
            alarms: vec![alarm_at(hour, minute, true)?],
        })
    }

    #[test]
    fn the_crc_is_ieee() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn a_slot_is_laid_out_as_adr_0011_d2_says() -> Result<(), Box<dyn std::error::Error>> {
        let choices = choices_at(6, 45)?;
        let slot = encode(&choices, 7)?;
        assert_eq!(&slot[..8], b"P20ALRM\0");
        assert_eq!(slot[8..12], 2_u32.to_le_bytes());
        assert_eq!(slot[12..16], 14_u32.to_le_bytes());
        assert_eq!(slot[16..24], 7_u64.to_le_bytes());
        assert_eq!(
            slot[24..38],
            [0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 1, 6, 45, 0b111_1111, 1]
        );
        assert_eq!(slot[38..42], crc32(&slot[..38]).to_le_bytes());
        assert!(slot[42..].iter().all(|&byte| byte == 0));
        Ok(())
    }

    #[test]
    fn a_first_version_slot_loads_with_the_power_off_on() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut record = FakeRecord::blank();
        let first = [0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 6, 45, 0b111_1111, 1];
        record.put(Slot::First, &framed_as(FIRST_VERSION, &first, 4)?);
        assert_eq!(load(&mut record)?, Some(choices_at(6, 45)?));
        save(&mut record, &choices_at(6, 45)?)?;
        assert_eq!(record.saved, [Slot::Second]);
        assert_eq!(record.bytes[SLOT_BYTES + 8], 2);
        Ok(())
    }

    #[test]
    fn the_power_off_turned_off_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let mut stays = choices_at(6, 45)?;
        stays.power_off = false;
        save(&mut record, &stays)?;
        assert_eq!(record.bytes[HEADER_BYTES + 8], 0);
        assert_eq!(load(&mut record)?, Some(stays));
        Ok(())
    }

    #[test]
    fn a_blank_record_was_never_written() -> Result<(), HardwareError> {
        assert_eq!(load(&mut FakeRecord::blank())?, None);
        Ok(())
    }

    #[test]
    fn the_first_save_goes_to_slot_zero_and_loads_back() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let choices = choices_at(6, 45)?;
        save(&mut record, &choices)?;
        assert_eq!(record.saved, [Slot::First]);
        assert_eq!(record.sequence(Slot::First), Some(0));
        assert_eq!(load(&mut record)?, Some(choices));
        Ok(())
    }

    #[test]
    fn saves_alternate_slots_and_the_higher_sequence_loads()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        for minute in [1, 2, 3] {
            save(&mut record, &choices_at(7, minute)?)?;
        }
        assert_eq!(record.saved, [Slot::First, Slot::Second, Slot::First]);
        assert_eq!(record.sequence(Slot::First), Some(2));
        assert_eq!(record.sequence(Slot::Second), Some(1));
        assert_eq!(load(&mut record)?, Some(choices_at(7, 3)?));
        Ok(())
    }

    #[test]
    fn a_torn_newer_slot_falls_back_to_the_older() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        save(&mut record, &choices_at(7, 1)?)?;
        save(&mut record, &choices_at(7, 2)?)?;
        record.bytes[SLOT_BYTES + 30] ^= 0xff;
        assert_eq!(load(&mut record)?, Some(choices_at(7, 1)?));
        save(&mut record, &choices_at(7, 3)?)?;
        assert_eq!(record.saved.last(), Some(&Slot::Second));
        assert_eq!(record.sequence(Slot::Second), Some(1));
        assert_eq!(load(&mut record)?, Some(choices_at(7, 3)?));
        Ok(())
    }

    #[test]
    fn a_save_cut_off_halfway_leaves_the_other_slot_whole() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut record = FakeRecord::blank();
        save(&mut record, &choices_at(7, 1)?)?;
        let half = encode(&choices_at(7, 2)?, 1)?;
        record.bytes[SLOT_BYTES..SLOT_BYTES + 20].copy_from_slice(&half[..20]);
        assert_eq!(load(&mut record)?, Some(choices_at(7, 1)?));
        Ok(())
    }

    #[test]
    fn both_slots_corrupt_is_a_hardware_error_and_the_next_save_takes_slot_zero()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        save(&mut record, &choices_at(7, 1)?)?;
        save(&mut record, &choices_at(7, 2)?)?;
        record.bytes[30] ^= 0xff;
        record.bytes[SLOT_BYTES + 30] ^= 0xff;
        assert!(matches!(
            load(&mut record),
            Err(HardwareError::Failed { device: DEVICE, .. })
        ));
        save(&mut record, &choices_at(7, 3)?)?;
        assert_eq!(record.saved.last(), Some(&Slot::First));
        assert_eq!(load(&mut record)?, Some(choices_at(7, 3)?));
        Ok(())
    }

    #[test]
    fn a_stray_byte_on_a_blank_record_is_not_never_written() {
        let mut record = FakeRecord::blank();
        record.bytes[RECORD_BYTES - 1] = 1;
        assert!(load(&mut record).is_err());
    }

    #[test]
    fn the_sequence_wraps_and_zero_is_newer_than_the_largest()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        record.put(Slot::First, &encode(&choices_at(7, 1)?, u64::MAX)?);
        assert_eq!(load(&mut record)?, Some(choices_at(7, 1)?));
        save(&mut record, &choices_at(7, 2)?)?;
        assert_eq!(record.saved, [Slot::Second]);
        assert_eq!(record.sequence(Slot::Second), Some(0));
        assert_eq!(load(&mut record)?, Some(choices_at(7, 2)?));
        save(&mut record, &choices_at(7, 3)?)?;
        assert_eq!(record.sequence(Slot::First), Some(1));
        assert_eq!(load(&mut record)?, Some(choices_at(7, 3)?));
        Ok(())
    }

    #[test]
    fn a_device_that_fails_is_passed_on_and_nothing_is_written() -> Result<(), TimeError> {
        let mut record = FakeRecord::blank();
        record.broken = true;
        assert_eq!(load(&mut record), Err(HardwareError::Absent("reserved4")));
        assert_eq!(
            save(&mut record, &choices_at(7, 1)?),
            Err(HardwareError::Absent("reserved4"))
        );
        assert_eq!(record.saved, []);
        Ok(())
    }

    #[test]
    fn a_field_out_of_range_invalidates_a_slot_whose_crc_holds()
    -> Result<(), Box<dyn std::error::Error>> {
        let good = payload(&choices_at(6, 45)?)?;
        let mut bad = Vec::new();
        for (at, value) in [
            (10, 24),
            (11, 60),
            (12, 0b1000_0000),
            (13, 2),
            (9, 2),
            (8, 2),
        ] {
            let mut payload = good.clone();
            payload[at] = value;
            bad.push(payload);
        }
        let mut offset = good.clone();
        offset[..4].copy_from_slice(&100_000_i32.to_le_bytes());
        bad.push(offset);
        let mut longer = good.clone();
        longer.push(0);
        bad.push(longer);
        bad.push(good[..9].to_vec());
        assert!(decode(&framed(&good, 0)?).is_some());
        for payload in bad {
            assert_eq!(decode(&framed(&payload, 0)?), None, "{payload:?}");
        }
        Ok(())
    }

    #[test]
    fn another_version_or_magic_or_length_invalidates_a_slot()
    -> Result<(), Box<dyn std::error::Error>> {
        let slot = encode(&choices_at(6, 45)?, 0)?;
        let mut version = slot;
        version[8] = 3;
        let mut magic = slot;
        magic[0] = b'Q';
        let mut length = slot;
        length[12..16].copy_from_slice(&u32::try_from(LARGEST_PAYLOAD + 1)?.to_le_bytes());
        let mut huge = slot;
        huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        for slot in [version, magic, length, huge] {
            assert_eq!(decode(&slot), None);
        }
        let mut unversioned = slot;
        unversioned[8] = 0;
        assert_eq!(decode(&unversioned), None);
        Ok(())
    }

    #[test]
    fn the_largest_payload_fills_the_slot_and_one_byte_more_is_refused() -> Result<(), HardwareError>
    {
        let slot = framed(&[0; LARGEST_PAYLOAD], 0)?;
        assert_eq!(
            slot[SLOT_BYTES - CRC_BYTES..],
            crc32(&slot[..SLOT_BYTES - CRC_BYTES]).to_le_bytes()
        );
        assert!(framed(&[0; LARGEST_PAYLOAD + 1], 0).is_err());
        Ok(())
    }

    #[test]
    fn choices_the_codec_cannot_hold_are_refused_before_a_write()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let mut window = choices_at(7, 1)?;
        window.window_seconds = -1;
        let mut many = choices_at(7, 1)?;
        many.alarms = vec![alarm_at(7, 1, false)?; 256];
        for choices in [window, many] {
            assert!(save(&mut record, &choices).is_err());
        }
        assert_eq!(record.saved, []);
        Ok(())
    }

    #[test]
    fn no_alarms_and_many_alarms_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let mut none = choices_at(7, 1)?;
        none.alarms.clear();
        save(&mut record, &none)?;
        assert_eq!(load(&mut record)?, Some(none));
        let mut many = choices_at(7, 1)?;
        many.alarms = vec![alarm_at(23, 59, false)?; 255];
        save(&mut record, &many)?;
        assert_eq!(load(&mut record)?, Some(many));
        Ok(())
    }
}
