use crate::alarm::{Alarm, Weekdays};
use crate::hardware::{HardwareError, Settings};
use crate::light::{LIT_CHOICES, LIT_SECONDS};
use crate::ringer::Outputs;
use crate::sound::Volume;
use crate::time::{Countdown, TimeOfDay, UtcOffset};

pub const RECORD_BYTES: usize = 2 * SLOT_BYTES;
pub const SLOT_BYTES: usize = 4096;
pub const MAGIC: [u8; 8] = *b"P20ALRM\0";
pub const VERSION: u32 = 6;
pub const FIRST_VERSION: u32 = 1;
const UNTIMED_VERSION: u32 = 2;
const UNLIT_VERSION: u32 = 3;
const UNVOLUMED_VERSION: u32 = 4;
const UNTOGGLED_VERSION: u32 = 5;
pub const LARGEST_PAYLOAD: usize = SLOT_BYTES - HEADER_BYTES - CRC_BYTES;

const HEADER_BYTES: usize = 24;
const CRC_BYTES: usize = 4;
const CHOICES_BYTES: usize = 10;
const FIRST_CHOICES_BYTES: usize = 9;
const ALARM_BYTES: usize = 4;
const TIMER_BYTES: usize = 2;
const LIT_BYTES: usize = 2;
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
    pub timers: Vec<Countdown>,
    pub lit_seconds: i64,
    pub volume: Volume,
    pub outputs: Outputs,
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
    let timers = u8::try_from(choices.timers.len())
        .map_err(|_| refused(format!("{} timers", choices.timers.len())))?;
    if let Some(zero) = choices.timers.iter().find(|timer| timer.is_zero()) {
        return Err(refused(format!("a timer of {zero:?}")));
    }
    let lit = u16::try_from(choices.lit_seconds)
        .ok()
        .filter(|_| LIT_CHOICES.contains(&choices.lit_seconds))
        .ok_or_else(|| refused(format!("a lit time of {} s", choices.lit_seconds)))?;
    let mut bytes = Vec::with_capacity(
        CHOICES_BYTES
            + ALARM_BYTES * choices.alarms.len()
            + 1
            + TIMER_BYTES * choices.timers.len()
            + LIT_BYTES
            + 3,
    );
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
    bytes.push(timers);
    for timer in &choices.timers {
        bytes.extend_from_slice(&[timer.minutes(), timer.seconds()]);
    }
    bytes.extend_from_slice(&lit.to_le_bytes());
    bytes.push(choices.volume.level());
    bytes.push(u8::from(choices.outputs.sound));
    bytes.push(u8::from(choices.outputs.vibration));
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

fn timer([minutes, seconds]: [u8; TIMER_BYTES]) -> Option<Countdown> {
    Countdown::new(minutes, seconds)
        .ok()
        .filter(|timer| !timer.is_zero())
}

fn listed<const N: usize, T>(
    bytes: &[u8],
    count: usize,
    entry: fn([u8; N]) -> Option<T>,
) -> Option<(Vec<T>, &[u8])> {
    let (entries, rest) = bytes.split_at_checked(count.checked_mul(N)?)?;
    let (entries, _) = entries.as_chunks::<N>();
    let entries = entries
        .iter()
        .copied()
        .map(entry)
        .collect::<Option<Vec<_>>>()?;
    Some((entries, rest))
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
    let (alarms, rest) = listed(payload.get(fixed..)?, count, set_alarm)?;
    let (timers, rest) = if version <= UNTIMED_VERSION {
        (Vec::new(), rest)
    } else {
        let (&count, rest) = rest.split_first()?;
        listed(rest, usize::from(count), timer)?
    };
    let (lit_seconds, rest) = if version <= UNLIT_VERSION {
        (LIT_SECONDS, rest)
    } else {
        let (lit, rest) = rest.split_first_chunk::<LIT_BYTES>()?;
        let lit = i64::from(u16::from_le_bytes(*lit));
        (Some(lit).filter(|lit| LIT_CHOICES.contains(lit))?, rest)
    };
    let (volume, rest) = if version <= UNVOLUMED_VERSION {
        (Volume::LOUDEST, rest)
    } else {
        let (&level, rest) = rest.split_first()?;
        (Volume::new(level)?, rest)
    };
    let (outputs, rest) = if version <= UNTOGGLED_VERSION {
        (Outputs::BOTH, rest)
    } else {
        let (&[sound, vibration], rest) = rest.split_first_chunk::<2>()?;
        let outputs = Outputs {
            sound: flag(sound)?,
            vibration: flag(vibration)?,
        };
        (outputs, rest)
    };
    if !rest.is_empty() {
        return None;
    }
    Some(Choices {
        offset,
        window_seconds: i64::from(window),
        power_off,
        alarms,
        timers,
        lit_seconds,
        volume,
        outputs,
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

    const QUIETER: Volume = Volume::LOUDEST.down().down();

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
            timers: vec![Countdown::new(5, 30)?],
            lit_seconds: 30,
            volume: QUIETER,
            outputs: Outputs {
                sound: false,
                vibration: true,
            },
        })
    }

    fn untoggled(hour: u8, minute: u8) -> Result<Choices, TimeError> {
        Ok(Choices {
            outputs: Outputs::BOTH,
            ..choices_at(hour, minute)?
        })
    }

    fn unvolumed(hour: u8, minute: u8) -> Result<Choices, TimeError> {
        Ok(Choices {
            volume: Volume::LOUDEST,
            ..untoggled(hour, minute)?
        })
    }

    fn unlit(hour: u8, minute: u8) -> Result<Choices, TimeError> {
        Ok(Choices {
            lit_seconds: LIT_SECONDS,
            ..unvolumed(hour, minute)?
        })
    }

    fn untimed(hour: u8, minute: u8) -> Result<Choices, TimeError> {
        Ok(Choices {
            timers: Vec::new(),
            ..unlit(hour, minute)?
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
        assert_eq!(slot[8..12], 6_u32.to_le_bytes());
        assert_eq!(slot[12..16], 22_u32.to_le_bytes());
        assert_eq!(slot[16..24], 7_u64.to_le_bytes());
        assert_eq!(
            slot[24..46],
            [
                0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 1, 6, 45, 0b111_1111, 1, 1, 5, 30, 30, 0, 13, 0,
                1
            ]
        );
        assert_eq!(slot[46..50], crc32(&slot[..46]).to_le_bytes());
        assert!(slot[50..].iter().all(|&byte| byte == 0));
        Ok(())
    }

    #[test]
    fn a_first_version_slot_loads_with_the_power_off_on() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut record = FakeRecord::blank();
        let first = [0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 6, 45, 0b111_1111, 1];
        record.put(Slot::First, &framed_as(FIRST_VERSION, &first, 4)?);
        assert_eq!(load(&mut record)?, Some(untimed(6, 45)?));
        save(&mut record, &choices_at(6, 45)?)?;
        assert_eq!(record.saved, [Slot::Second]);
        assert_eq!(record.bytes[SLOT_BYTES + 8], 6);
        assert_eq!(load(&mut record)?, Some(choices_at(6, 45)?));
        Ok(())
    }

    #[test]
    fn a_second_version_slot_loads_with_no_timers() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let second = [0x10, 0x0e, 0, 0, 120, 0, 0, 0, 0, 1, 6, 45, 0b111_1111, 1];
        record.put(Slot::Second, &framed_as(UNTIMED_VERSION, &second, 9)?);
        let stays = Choices {
            power_off: false,
            ..untimed(6, 45)?
        };
        assert_eq!(load(&mut record)?, Some(stays));
        let mut timed = second.to_vec();
        timed.extend_from_slice(&[1, 5, 30]);
        assert_eq!(decode(&framed_as(UNTIMED_VERSION, &timed, 0)?), None);
        Ok(())
    }

    #[test]
    fn a_third_version_slot_loads_lit_for_the_default() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let third = [
            0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 1, 6, 45, 0b111_1111, 1, 1, 5, 30,
        ];
        record.put(Slot::First, &framed_as(UNLIT_VERSION, &third, 2)?);
        assert_eq!(load(&mut record)?, Some(unlit(6, 45)?));
        let mut lit = third.to_vec();
        lit.extend_from_slice(&[30, 0]);
        assert_eq!(decode(&framed_as(UNLIT_VERSION, &lit, 0)?), None);
        save(&mut record, &choices_at(6, 45)?)?;
        assert_eq!(record.bytes[SLOT_BYTES + 8], 6);
        assert_eq!(load(&mut record)?, Some(choices_at(6, 45)?));
        Ok(())
    }

    #[test]
    fn a_fourth_version_slot_loads_at_the_loudest_volume() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut record = FakeRecord::blank();
        let fourth = [
            0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 1, 6, 45, 0b111_1111, 1, 1, 5, 30, 30, 0,
        ];
        record.put(Slot::First, &framed_as(UNVOLUMED_VERSION, &fourth, 2)?);
        assert_eq!(load(&mut record)?, Some(unvolumed(6, 45)?));
        let mut volumed = fourth.to_vec();
        volumed.push(13);
        assert_eq!(decode(&framed_as(UNVOLUMED_VERSION, &volumed, 0)?), None);
        save(&mut record, &choices_at(6, 45)?)?;
        assert_eq!(record.bytes[SLOT_BYTES + 8], 6);
        assert_eq!(load(&mut record)?, Some(choices_at(6, 45)?));
        Ok(())
    }

    #[test]
    fn a_fifth_version_slot_loads_with_the_sound_and_the_vibration_on()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let fifth = [
            0x10, 0x0e, 0, 0, 120, 0, 0, 0, 1, 1, 6, 45, 0b111_1111, 1, 1, 5, 30, 30, 0, 13,
        ];
        record.put(Slot::First, &framed_as(UNTOGGLED_VERSION, &fifth, 2)?);
        let loaded = Choices {
            volume: QUIETER,
            ..untoggled(6, 45)?
        };
        assert_eq!(load(&mut record)?, Some(loaded));
        let mut toggled = fifth.to_vec();
        toggled.extend_from_slice(&[0, 1]);
        assert_eq!(decode(&framed_as(UNTOGGLED_VERSION, &toggled, 0)?), None);
        save(&mut record, &choices_at(6, 45)?)?;
        assert_eq!(record.bytes[SLOT_BYTES + 8], 6);
        assert_eq!(load(&mut record)?, Some(choices_at(6, 45)?));
        Ok(())
    }

    #[test]
    fn every_pair_of_sound_and_vibration_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        for (sound, vibration) in [(true, true), (true, false), (false, true), (false, false)] {
            let choices = Choices {
                outputs: Outputs { sound, vibration },
                ..choices_at(6, 45)?
            };
            save(&mut record, &choices)?;
            assert_eq!(load(&mut record)?, Some(choices));
        }
        Ok(())
    }

    #[test]
    fn every_volume_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        for level in 1..=15 {
            let choices = Choices {
                volume: Volume::new(level).ok_or("no volume")?,
                ..choices_at(6, 45)?
            };
            save(&mut record, &choices)?;
            assert_eq!(load(&mut record)?, Some(choices));
        }
        Ok(())
    }

    #[test]
    fn every_lit_choice_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        for lit_seconds in LIT_CHOICES {
            let choices = Choices {
                lit_seconds,
                ..choices_at(6, 45)?
            };
            save(&mut record, &choices)?;
            assert_eq!(load(&mut record)?, Some(choices));
        }
        Ok(())
    }

    #[test]
    fn every_saved_timer_round_trips_in_order() -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FakeRecord::blank();
        let mut timers = choices_at(6, 45)?;
        timers.timers = vec![
            Countdown::new(0, 1)?,
            Countdown::new(3, 0)?,
            Countdown::new(12, 5)?,
            Countdown::LONGEST,
        ];
        save(&mut record, &timers)?;
        assert_eq!(load(&mut record)?, Some(timers));
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
            (14, 2),
            (14, 0),
            (15, 100),
            (16, 60),
            (17, 11),
            (18, 1),
            (19, 0),
            (19, 16),
            (20, 2),
            (21, 2),
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
        bad.push(good[..14].to_vec());
        bad.push(good[..16].to_vec());
        bad.push(good[..17].to_vec());
        bad.push(good[..18].to_vec());
        bad.push(good[..19].to_vec());
        bad.push(good[..20].to_vec());
        bad.push(good[..21].to_vec());
        let mut zero = good.clone();
        zero[15..17].copy_from_slice(&[0, 0]);
        bad.push(zero);
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
        version[8] = 7;
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
        let mut zero = choices_at(7, 1)?;
        zero.timers = vec![Countdown::default()];
        let mut unlisted = choices_at(7, 1)?;
        unlisted.lit_seconds = 45;
        let mut huge = choices_at(7, 1)?;
        huge.lit_seconds = i64::from(u16::MAX) + 30;
        for choices in [window, many, zero, unlisted, huge] {
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
