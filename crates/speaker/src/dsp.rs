use alarm_core::hardware::HardwareError;

use crate::{failed, put, word};

pub const ASYNC: u32 = 0xc00c_4170;
pub const SYNC: u32 = 0x4018_4171;
pub const WRITE_PARAMS: u32 = 0xc018_4175;
pub const ASYNC_BYTES: usize = 12;
pub const SYNC_BYTES: usize = 24;
pub const LARGEST_BODY: u32 = 4 << 20;

const DEVICE: &str = "dsp-init.bin";
const FRAME_BYTES: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Async,
    Sync,
    WriteParams,
}

impl Kind {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Async),
            1 => Some(Self::Sync),
            2 => Some(Self::WriteParams),
            _ => None,
        }
    }

    #[must_use]
    pub fn request(self) -> u32 {
        match self {
            Self::Async => ASYNC,
            Self::Sync => SYNC,
            Self::WriteParams => WRITE_PARAMS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: Kind,
    pub body: Vec<u8>,
    pub reply: u32,
}

fn length(at: usize, name: &str, found: u32) -> Result<usize, HardwareError> {
    if found > LARGEST_BODY {
        return Err(failed(
            DEVICE,
            format!("a {name} of {found} bytes at byte {at}"),
        ));
    }
    usize::try_from(found).map_err(|error| failed(DEVICE, error.to_string()))
}

pub fn messages(bytes: &[u8]) -> Result<Vec<Message>, HardwareError> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let kind = Kind::from_byte(bytes[at])
            .ok_or_else(|| failed(DEVICE, format!("kind {} at byte {at}", bytes[at])))?;
        let body = length(at, "body", word(DEVICE, bytes, at + 1)?)?;
        let reply = word(DEVICE, bytes, at + 5)?;
        length(at, "reply", reply)?;
        if body == 0 || (kind == Kind::Async && reply != 0) {
            return Err(failed(
                DEVICE,
                format!("a {kind:?} message of {body} bytes and {reply} back at byte {at}"),
            ));
        }
        let start = at + FRAME_BYTES;
        let body = bytes
            .get(start..start + body)
            .ok_or_else(|| failed(DEVICE, format!("a body past the end at byte {at}")))?;
        found.push(Message {
            kind,
            body: body.to_vec(),
            reply,
        });
        at = start + body.len();
    }
    if found.is_empty() {
        return Err(failed(DEVICE, "no messages".to_owned()));
    }
    Ok(found)
}

#[must_use]
pub fn argument(kind: Kind, body: u64, length: u32, reply: u64, reply_length: u32) -> Vec<u8> {
    let mut bytes = vec![
        0;
        match kind {
            Kind::Async => ASYNC_BYTES,
            Kind::Sync | Kind::WriteParams => SYNC_BYTES,
        }
    ];
    put(&mut bytes, 0, &body.to_le_bytes());
    put(&mut bytes, 8, &length.to_le_bytes());
    if kind != Kind::Async {
        put(&mut bytes, 12, &reply.to_le_bytes());
        put(&mut bytes, 20, &reply_length.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoded;

    fn frame(kind: u8, body: &[u8], reply: u32) -> Vec<u8> {
        let mut bytes = vec![kind];
        bytes.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_le_bytes());
        bytes.extend_from_slice(&reply.to_le_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    const PLAY_PAUSE: [u8; 8] = [0x53, 0xdd, 0, 0, 0, 0, 0, 0];
    const SET_PARA: [u8; 16] = [
        0x08, 0xdf, 0x07, 0x00, 0x01, 0, 0, 0, 0x04, 0, 0, 0, 0x04, 0, 0, 0x80,
    ];

    #[test]
    fn the_requests_encode_their_argument_sizes() {
        assert_eq!(encoded(3, b'A', 0x70, ASYNC_BYTES), Some(ASYNC));
        assert_eq!(encoded(1, b'A', 0x71, SYNC_BYTES), Some(SYNC));
        assert_eq!(encoded(3, b'A', 0x75, SYNC_BYTES), Some(WRITE_PARAMS));
    }

    #[test]
    fn the_init_is_split_into_its_messages_in_order() -> Result<(), HardwareError> {
        let mut bytes = frame(1, &PLAY_PAUSE, 8);
        bytes.extend(frame(2, &[0; 32], 0));
        bytes.extend(frame(0, &SET_PARA, 0));
        assert_eq!(
            messages(&bytes)?,
            [
                Message {
                    kind: Kind::Sync,
                    body: PLAY_PAUSE.to_vec(),
                    reply: 8,
                },
                Message {
                    kind: Kind::WriteParams,
                    body: vec![0; 32],
                    reply: 0,
                },
                Message {
                    kind: Kind::Async,
                    body: SET_PARA.to_vec(),
                    reply: 0,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn a_malformed_init_is_a_hardware_error() {
        let whole = frame(0, &SET_PARA, 0);
        let mut oversized = frame(2, &[0; 4], 0);
        oversized[1..5].copy_from_slice(&(LARGEST_BODY + 1).to_le_bytes());
        for bytes in [
            Vec::new(),
            frame(3, &SET_PARA, 0),
            frame(0, &SET_PARA, 4),
            frame(1, &[], 8),
            frame(1, &PLAY_PAUSE, LARGEST_BODY + 1),
            oversized,
            whole[..whole.len() - 1].to_vec(),
            whole[..FRAME_BYTES - 1].to_vec(),
        ] {
            assert!(
                matches!(
                    messages(&bytes),
                    Err(HardwareError::Failed { device: DEVICE, .. })
                ),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn an_async_argument_is_the_body_and_its_length() {
        let bytes = argument(Kind::Async, 0x71_9b51_e080, 16, 0x1234, 8);
        assert_eq!(bytes, [0x80, 0xe0, 0x51, 0x9b, 0x71, 0, 0, 0, 16, 0, 0, 0]);
    }

    #[test]
    fn a_sync_argument_adds_the_reply_and_its_length() {
        for kind in [Kind::Sync, Kind::WriteParams] {
            let bytes = argument(kind, 0x71_9b51_e080, 8, 0x72_0000_0010, 8);
            assert_eq!(
                bytes,
                [
                    0x80, 0xe0, 0x51, 0x9b, 0x71, 0, 0, 0, 8, 0, 0, 0, 0x10, 0, 0, 0, 0x72, 0, 0,
                    0, 8, 0, 0, 0
                ]
            );
        }
    }
}
