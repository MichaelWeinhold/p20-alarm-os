use alarm_core::hardware::HardwareError;

use crate::{failed, put, word};

pub const ELEM_LIST: u32 = 0xc050_5510;
pub const ELEM_INFO: u32 = 0xc110_5511;
pub const ELEM_WRITE: u32 = 0xc4c8_5513;
pub const ID_BYTES: usize = 64;
pub const LIST_BYTES: usize = 80;
pub const INFO_BYTES: usize = 272;
pub const VALUE_BYTES: usize = 1224;
pub const MAX_VALUES: usize = 128;
pub const MAX_ITEMS: u32 = 256;
pub const BOOLEAN: u32 = 1;
pub const INTEGER: u32 = 2;
pub const ENUMERATED: u32 = 3;

pub(crate) const DEVICE: &str = "controlC0";
const NAME_AT: usize = 16;
const NAME_BYTES: usize = 44;
const LIST_SPACE: usize = 4;
const LIST_USED: usize = 8;
const LIST_COUNT: usize = 12;
const LIST_IDS: usize = 16;
const INFO_TYPE: usize = 64;
const INFO_COUNT: usize = 72;
const INFO_ITEMS: usize = 80;
const INFO_ITEM: usize = 84;
const INFO_ITEM_NAME: usize = 88;
const ITEM_NAME_BYTES: usize = 64;
const VALUES_AT: usize = 72;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub numid: u32,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub kind: u32,
    pub count: usize,
    pub items: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Integer(i64),
    Item(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Write {
    pub numid: u32,
    pub count: usize,
    pub setting: Setting,
}

#[must_use]
pub fn list(space: u32, ids: u64) -> Vec<u8> {
    let mut bytes = vec![0; LIST_BYTES];
    put(&mut bytes, LIST_SPACE, &space.to_le_bytes());
    put(&mut bytes, LIST_IDS, &ids.to_le_bytes());
    bytes
}

pub fn listed(bytes: &[u8]) -> Result<(u32, u32), HardwareError> {
    Ok((
        word(DEVICE, bytes, LIST_USED)?,
        word(DEVICE, bytes, LIST_COUNT)?,
    ))
}

fn text(bytes: &[u8]) -> Result<String, HardwareError> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec())
        .map_err(|error| failed(DEVICE, format!("a name that is not UTF-8: {error}")))
}

pub fn elements(ids: &[u8]) -> Result<Vec<Element>, HardwareError> {
    let (whole, rest) = ids.as_chunks::<ID_BYTES>();
    if !rest.is_empty() {
        return Err(failed(DEVICE, format!("{} bytes of ids", ids.len())));
    }
    whole
        .iter()
        .map(|id| {
            Ok(Element {
                numid: word(DEVICE, id, 0)?,
                name: text(&id[NAME_AT..NAME_AT + NAME_BYTES])?,
            })
        })
        .collect()
}

#[must_use]
pub fn info(numid: u32, item: u32) -> Vec<u8> {
    let mut bytes = vec![0; INFO_BYTES];
    put(&mut bytes, 0, &numid.to_le_bytes());
    put(&mut bytes, INFO_ITEM, &item.to_le_bytes());
    bytes
}

pub fn described(name: &str, bytes: &[u8]) -> Result<Info, HardwareError> {
    let kind = word(DEVICE, bytes, INFO_TYPE)?;
    let count = usize::try_from(word(DEVICE, bytes, INFO_COUNT)?)
        .map_err(|error| failed(DEVICE, error.to_string()))?;
    if count == 0 || count > MAX_VALUES {
        return Err(failed(DEVICE, format!("{name} holds {count} values")));
    }
    let items = if kind == ENUMERATED {
        word(DEVICE, bytes, INFO_ITEMS)?
    } else {
        0
    };
    if kind == ENUMERATED && (items == 0 || items > MAX_ITEMS) {
        return Err(failed(DEVICE, format!("{name} offers {items} items")));
    }
    Ok(Info { kind, count, items })
}

pub fn item_name(bytes: &[u8]) -> Result<String, HardwareError> {
    let name = bytes
        .get(INFO_ITEM_NAME..INFO_ITEM_NAME + ITEM_NAME_BYTES)
        .ok_or_else(|| failed(DEVICE, format!("an info of {} bytes", bytes.len())))?;
    text(name)
}

pub fn setting(
    name: &str,
    info: &Info,
    items: &[String],
    value: &str,
) -> Result<Setting, HardwareError> {
    let refused = || failed(DEVICE, format!("{name} takes no {value:?}"));
    match info.kind {
        BOOLEAN => match value {
            "0" => Ok(Setting::Integer(0)),
            "1" => Ok(Setting::Integer(1)),
            _ if value.eq_ignore_ascii_case("off") => Ok(Setting::Integer(0)),
            _ if value.eq_ignore_ascii_case("on") => Ok(Setting::Integer(1)),
            _ => Err(refused()),
        },
        INTEGER => value.parse().map(Setting::Integer).map_err(|_| refused()),
        ENUMERATED => items
            .iter()
            .position(|item| item == value)
            .and_then(|index| u32::try_from(index).ok())
            .map(Setting::Item)
            .ok_or_else(refused),
        kind => Err(failed(DEVICE, format!("{name} is of type {kind}"))),
    }
}

#[must_use]
pub fn value(write: &Write) -> Vec<u8> {
    let mut bytes = vec![0; VALUE_BYTES];
    put(&mut bytes, 0, &write.numid.to_le_bytes());
    for index in 0..write.count.min(MAX_VALUES) {
        match write.setting {
            Setting::Integer(value) => put(&mut bytes, VALUES_AT + index * 8, &value.to_le_bytes()),
            Setting::Item(item) => put(&mut bytes, VALUES_AT + index * 4, &item.to_le_bytes()),
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoded;

    fn id(numid: u32, name: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; ID_BYTES];
        bytes[..4].copy_from_slice(&numid.to_le_bytes());
        bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
        bytes[NAME_AT..NAME_AT + name.len()].copy_from_slice(name);
        bytes
    }

    fn info_reply(kind: u32, count: u32, items: u32, item_name: &[u8]) -> Vec<u8> {
        let mut bytes = info(7, 0);
        bytes[INFO_TYPE..INFO_TYPE + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[INFO_COUNT..INFO_COUNT + 4].copy_from_slice(&count.to_le_bytes());
        bytes[INFO_ITEMS..INFO_ITEMS + 4].copy_from_slice(&items.to_le_bytes());
        bytes[INFO_ITEM_NAME..INFO_ITEM_NAME + item_name.len()].copy_from_slice(item_name);
        bytes
    }

    fn items(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn the_requests_encode_the_layouts_of_their_structs() {
        assert_eq!(encoded(3, b'U', 0x10, LIST_BYTES), Some(ELEM_LIST));
        assert_eq!(encoded(3, b'U', 0x11, INFO_BYTES), Some(ELEM_INFO));
        assert_eq!(encoded(3, b'U', 0x13, VALUE_BYTES), Some(ELEM_WRITE));
        assert_eq!(NAME_AT + NAME_BYTES + 4, ID_BYTES);
        assert_eq!(ID_BYTES + 16 + 128 + 64, INFO_BYTES);
        assert_eq!(VALUES_AT + MAX_VALUES * 8 + 128, VALUE_BYTES);
        assert_eq!(LIST_IDS + 8 + 50 + 6, LIST_BYTES);
    }

    #[test]
    fn a_list_asks_for_space_at_an_address_and_reads_back_its_counts() -> Result<(), HardwareError>
    {
        let mut bytes = list(321, 0x71_9b51_e080);
        assert_eq!(bytes[LIST_SPACE..LIST_SPACE + 4], 321_u32.to_le_bytes());
        assert_eq!(
            bytes[LIST_IDS..LIST_IDS + 8],
            0x71_9b51_e080_u64.to_le_bytes()
        );
        bytes[LIST_USED..LIST_USED + 4].copy_from_slice(&300_u32.to_le_bytes());
        bytes[LIST_COUNT..LIST_COUNT + 4].copy_from_slice(&321_u32.to_le_bytes());
        assert_eq!(listed(&bytes)?, (300, 321));
        Ok(())
    }

    #[test]
    fn the_ids_give_each_element_its_number_and_name() -> Result<(), HardwareError> {
        let mut ids = id(1, b"DACL PGA");
        ids.extend(id(271, b"CLASSD VIR_SWITCH CLASSD_EN"));
        ids.extend(id(9, &[b'x'; NAME_BYTES]));
        assert_eq!(
            elements(&ids)?,
            [
                Element {
                    numid: 1,
                    name: "DACL PGA".to_owned(),
                },
                Element {
                    numid: 271,
                    name: "CLASSD VIR_SWITCH CLASSD_EN".to_owned(),
                },
                Element {
                    numid: 9,
                    name: "x".repeat(NAME_BYTES),
                },
            ]
        );
        assert!(elements(&ids[..=ID_BYTES]).is_err());
        assert!(elements(&id(2, &[0xff, 0xfe])).is_err());
        Ok(())
    }

    #[test]
    fn an_info_asks_by_number_and_item_and_reads_back_the_type() -> Result<(), HardwareError> {
        let asked = info(7, 3);
        assert_eq!(asked[..4], 7_u32.to_le_bytes());
        assert_eq!(asked[INFO_ITEM..INFO_ITEM + 4], 3_u32.to_le_bytes());
        assert_eq!(
            described("MUX", &info_reply(ENUMERATED, 1, 4, b"APB"))?,
            Info {
                kind: ENUMERATED,
                count: 1,
                items: 4,
            }
        );
        assert_eq!(item_name(&info_reply(ENUMERATED, 1, 4, b"APB"))?, "APB");
        assert_eq!(
            described("PGA", &info_reply(INTEGER, 2, 9, b""))?,
            Info {
                kind: INTEGER,
                count: 2,
                items: 0,
            }
        );
        for reply in [
            info_reply(INTEGER, 0, 0, b""),
            info_reply(INTEGER, 129, 0, b""),
            info_reply(ENUMERATED, 1, 0, b""),
            info_reply(ENUMERATED, 1, MAX_ITEMS + 1, b""),
        ] {
            assert!(described("bad", &reply).is_err());
        }
        Ok(())
    }

    #[test]
    fn a_value_is_set_by_its_type() -> Result<(), HardwareError> {
        let boolean = Info {
            kind: BOOLEAN,
            count: 1,
            items: 0,
        };
        let integer = Info {
            kind: INTEGER,
            ..boolean
        };
        let enumerated = Info {
            kind: ENUMERATED,
            count: 1,
            items: 3,
        };
        let names = items(&["Off", "APB", "U_3RD"]);
        assert_eq!(setting("A", &boolean, &[], "1")?, Setting::Integer(1));
        assert_eq!(setting("A", &boolean, &[], "Off")?, Setting::Integer(0));
        assert_eq!(setting("A", &boolean, &[], "on")?, Setting::Integer(1));
        assert_eq!(setting("A", &integer, &[], "5200")?, Setting::Integer(5200));
        assert_eq!(setting("A", &integer, &[], "-3")?, Setting::Integer(-3));
        assert_eq!(setting("A", &enumerated, &names, "APB")?, Setting::Item(1));
        for (info, value) in [
            (boolean, "2"),
            (boolean, "Offset"),
            (integer, "Off"),
            (integer, "APB"),
            (enumerated, "apb"),
            (enumerated, "1"),
        ] {
            assert!(setting("A", &info, &names, value).is_err(), "{value}");
        }
        let bytes = Info { kind: 4, ..boolean };
        assert!(setting("A", &bytes, &[], "0").is_err());
        Ok(())
    }

    #[test]
    fn a_write_sets_every_value_of_the_element() {
        let bytes = value(&Write {
            numid: 12,
            count: 2,
            setting: Setting::Integer(-1),
        });
        assert_eq!(bytes.len(), VALUE_BYTES);
        assert_eq!(bytes[..4], 12_u32.to_le_bytes());
        assert_eq!(bytes[VALUES_AT..VALUES_AT + 16], [0xff; 16]);
        assert_eq!(bytes[VALUES_AT + 16..VALUES_AT + 24], [0; 8]);
        let bytes = value(&Write {
            numid: 13,
            count: 2,
            setting: Setting::Item(5),
        });
        assert_eq!(
            bytes[VALUES_AT..VALUES_AT + 12],
            [5, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
