//! Version 1 Postcard messages in bounded, checksummed UDP-size frames.
//! Checksums detect corruption; peer authentication is outside this protocol.
use crate::sync::Message;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FRAME_LIMIT: usize = 1_200;
const HEADER: usize = 56;
const PAYLOAD: usize = FRAME_LIMIT - HEADER;
pub const MESSAGE_LIMIT: usize = 8 * 1024 * 1024;
const MAX_PARTS: usize = MESSAGE_LIMIT.div_ceil(PAYLOAD);

#[derive(Debug, PartialEq, Eq)]
pub enum WireError {
    InvalidFrame,
    WrongRoom,
    TooLarge,
    Capacity,
    Corrupt,
    InvalidMessage,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u8,
    message: Message,
}

pub fn encode(
    room: u128,
    sender: u128,
    id: u64,
    message: &Message,
) -> Result<Vec<Vec<u8>>, WireError> {
    let bytes = postcard::to_allocvec(&Envelope {
        version: 1,
        message: message.clone(),
    })
    .map_err(|_| WireError::InvalidMessage)?;
    if bytes.len() > MESSAGE_LIMIT {
        return Err(WireError::TooLarge);
    }
    let total = bytes.len().div_ceil(PAYLOAD) as u32;
    let checksum = crc32fast::hash(&bytes);
    Ok(bytes
        .chunks(PAYLOAD)
        .enumerate()
        .map(|(index, chunk)| {
            let mut frame = Vec::with_capacity(HEADER + chunk.len());
            frame.extend_from_slice(b"CV01");
            frame.extend_from_slice(&room.to_le_bytes());
            frame.extend_from_slice(&sender.to_le_bytes());
            frame.extend_from_slice(&id.to_le_bytes());
            frame.extend_from_slice(&total.to_le_bytes());
            frame.extend_from_slice(&(index as u32).to_le_bytes());
            frame.extend_from_slice(&checksum.to_le_bytes());
            frame.extend_from_slice(chunk);
            frame
        })
        .collect())
}

#[derive(Debug)]
struct Pending {
    started: u64,
    checksum: u32,
    parts: Vec<Option<Vec<u8>>>,
    bytes: usize,
}

#[derive(Debug)]
pub struct Reassembler {
    room: u128,
    pending: BTreeMap<(u128, u64), Pending>,
    bytes: usize,
}

impl Reassembler {
    pub fn new(room: u128) -> Self {
        Self {
            room,
            pending: BTreeMap::new(),
            bytes: 0,
        }
    }

    pub fn expire(&mut self, now_ms: u64) {
        self.pending.retain(|_, pending| {
            let keep = now_ms.saturating_sub(pending.started) < 10_000;
            if !keep {
                self.bytes -= pending.bytes;
            }
            keep
        });
    }

    pub fn receive(
        &mut self,
        now_ms: u64,
        frame: &[u8],
    ) -> Result<Option<(u128, Message)>, WireError> {
        self.expire(now_ms);
        if frame.len() <= HEADER || frame.len() > FRAME_LIMIT || &frame[..4] != b"CV01" {
            return Err(WireError::InvalidFrame);
        }
        let room = u128::from_le_bytes(frame[4..20].try_into().unwrap());
        if room != self.room {
            return Err(WireError::WrongRoom);
        }
        let sender = u128::from_le_bytes(frame[20..36].try_into().unwrap());
        let id = u64::from_le_bytes(frame[36..44].try_into().unwrap());
        let total = u32::from_le_bytes(frame[44..48].try_into().unwrap()) as usize;
        let index = u32::from_le_bytes(frame[48..52].try_into().unwrap()) as usize;
        let checksum = u32::from_le_bytes(frame[52..56].try_into().unwrap());
        if total == 0 || total > MAX_PARTS || index >= total {
            return Err(WireError::InvalidFrame);
        }
        if index + 1 < total && frame.len() != FRAME_LIMIT {
            return Err(WireError::InvalidFrame);
        }
        let key = (sender, id);
        if !self.pending.contains_key(&key) && self.pending.len() >= 8 {
            return Err(WireError::Capacity);
        }
        let pending = self.pending.entry(key).or_insert_with(|| Pending {
            started: now_ms,
            checksum,
            parts: vec![None; total],
            bytes: 0,
        });
        if pending.checksum != checksum || pending.parts.len() != total {
            return Err(WireError::Corrupt);
        }
        if let Some(existing) = &pending.parts[index] {
            if existing.as_slice() != &frame[HEADER..] {
                return Err(WireError::Corrupt);
            }
            return Ok(None);
        }
        let len = frame.len() - HEADER;
        if self.bytes + len > MESSAGE_LIMIT {
            return Err(WireError::Capacity);
        }
        pending.parts[index] = Some(frame[HEADER..].to_vec());
        pending.bytes += len;
        self.bytes += len;
        if pending.parts.iter().any(Option::is_none) {
            return Ok(None);
        }
        let pending = self.pending.remove(&key).unwrap();
        self.bytes -= pending.bytes;
        let bytes: Vec<u8> = pending.parts.into_iter().flatten().flatten().collect();
        if crc32fast::hash(&bytes) != checksum {
            return Err(WireError::Corrupt);
        }
        let (envelope, rest): (Envelope, _) =
            postcard::take_from_bytes(&bytes).map_err(|_| WireError::InvalidMessage)?;
        if envelope.version != 1 || !rest.is_empty() {
            return Err(WireError::InvalidMessage);
        }
        Ok(Some((sender, envelope.message)))
    }
}
