//! Browser bindings for the same replication engine as native peers.
use conv_protocol::outbox::{Outbox, control};
use conv_protocol::{
    replication::Replica,
    sync::{Effect, Message, SyncConfig, SyncPeer},
    wire::{Reassembler, encode},
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn error(value: impl std::fmt::Debug) -> JsValue {
    JsValue::from_str(&format!("{value:?}"))
}
fn parse_id(value: &str) -> Result<u128, JsValue> {
    u128::from_str_radix(value, 16).map_err(error)
}

#[derive(Serialize)]
struct Outbound {
    to: String,
    data: Vec<u8>,
    control: bool,
}
#[derive(Serialize)]
struct Update {
    text: String,
    history: usize,
    outbound: Vec<Outbound>,
}

#[wasm_bindgen]
pub struct Engine {
    peer: SyncPeer,
    room: u128,
    writer: u128,
    incoming: Reassembler,
    counter: u64,
    outbox: Outbox,
    batching: bool,
    include_text: bool,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(room: &str, writer: &str, backup: Option<Vec<u8>>) -> Result<Engine, JsValue> {
        let room = parse_id(room)?;
        let writer = parse_id(writer)?;
        let replica = match backup {
            Some(bytes) => Replica::restore(writer, room, &bytes).map_err(error)?,
            None => Replica::new(writer),
        };
        let mut peer = SyncPeer::from_replica(replica, Vec::new(), SyncConfig::default());
        peer.use_full_mesh_room();
        Ok(Self {
            peer,
            room,
            writer,
            incoming: Reassembler::new(room),
            counter: 0,
            outbox: Outbox::default(),
            batching: true,
            include_text: true,
        })
    }
    pub fn merge_backup(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.peer.merge_backup(self.room, bytes).map_err(error)
    }
    pub fn backup_room(bytes: &[u8]) -> Result<String, JsValue> {
        Ok(format!(
            "{:032x}",
            conv_protocol::backup::room_id(bytes).map_err(error)?
        ))
    }
    pub fn editor_lane(&mut self) {
        self.counter = 1_u64 << 63;
        self.batching = false;
        self.include_text = true;
    }
    /// The protocol worker never paints text. Avoid rebuilding the document on
    /// every control packet and timer; the editor lane renders accepted changes.
    pub fn protocol_lane(&mut self) {
        self.include_text = false;
    }
    pub fn history_len(&self) -> usize {
        self.peer.replica().history_len()
    }
    pub fn anchor(&self, offset: usize) -> Result<Vec<u8>, JsValue> {
        let anchor = self
            .peer
            .replica()
            .document()
            .anchor_utf16(offset)
            .ok_or_else(|| error("invalid cursor boundary"))?;
        postcard::to_allocvec(&anchor).map_err(error)
    }
    pub fn resolve_anchor(&self, bytes: &[u8]) -> Result<usize, JsValue> {
        let anchor = postcard::from_bytes(bytes).map_err(error)?;
        Ok(self.peer.replica().document().resolve_anchor(anchor))
    }
    pub fn needs_probe_order(&self) -> bool {
        self.peer.needs_probe_order()
    }
    pub fn reshuffle_probes(&mut self, ids: JsValue) -> Result<(), JsValue> {
        let ids: Vec<String> = serde_wasm_bindgen::from_value(ids).map_err(error)?;
        self.peer.reshuffle_probes(
            ids.iter()
                .map(|id| parse_id(id))
                .collect::<Result<_, _>>()?,
        );
        Ok(())
    }
    pub fn text(&self) -> String {
        self.peer.replica().document().text()
    }
    pub fn backup(&self) -> Result<Vec<u8>, JsValue> {
        self.peer.replica().backup(self.room).map_err(error)
    }
    pub fn connect(&mut self, ids: JsValue) -> Result<(), JsValue> {
        let ids: Vec<String> = serde_wasm_bindgen::from_value(ids).map_err(error)?;
        if ids.len() > 8 {
            return Err(error("browser overlay exceeds eight direct links"));
        }
        self.peer.set_neighbors(
            ids.iter()
                .map(|id| parse_id(id))
                .collect::<Result<_, _>>()?,
        );
        Ok(())
    }
    pub fn overlay(&mut self, enabled: bool) {
        if enabled {
            self.peer.use_overlay_room();
        } else {
            self.peer.use_full_mesh_room();
        }
    }
    pub fn edit(&mut self, start: usize, end: usize, text: &str) -> Result<JsValue, JsValue> {
        let mut candidate = self.peer.clone();
        let effects = candidate.edit_utf16(start, end, text).map_err(error)?;
        let sends = effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Send { .. }))
            .count() as u64;
        self.counter
            .checked_add(sends)
            .ok_or_else(|| error("message counter exhausted"))?;
        for effect in &effects {
            if let Effect::Send { message, .. } = effect {
                encode(self.room, self.writer, 0, message).map_err(error)?;
            }
        }
        self.peer = candidate;
        self.output(effects)
    }
    pub fn hello(&mut self, to: &str) -> Result<JsValue, JsValue> {
        self.output(vec![Effect::Send {
            to: parse_id(to)?,
            message: Message::Hello,
        }])
    }
    pub fn receive(&mut self, frame: &[u8], now_ms: u64) -> Result<JsValue, JsValue> {
        let effects = match self.incoming.receive(now_ms, frame).map_err(error)? {
            Some((from, message)) => self.peer.receive_at(from, message, now_ms).map_err(error)?,
            None => Vec::new(),
        };
        self.output(effects)
    }
    pub fn tick(&mut self, now_ms: u64, sync: bool) -> Result<JsValue, JsValue> {
        let mut effects = self.peer.on_membership_timer(now_ms);
        if sync {
            effects.extend(self.peer.on_sync_timer());
        }
        self.incoming.expire(now_ms);
        let mut immediate = self.outbox.push(effects);
        immediate.extend(self.outbox.flush());
        self.encode_output(immediate)
    }
}
impl Engine {
    fn output(&mut self, effects: Vec<Effect>) -> Result<JsValue, JsValue> {
        let effects = if self.batching {
            self.outbox.push(effects)
        } else {
            effects
        };
        self.encode_output(effects)
    }
    fn encode_output(&mut self, effects: Vec<Effect>) -> Result<JsValue, JsValue> {
        let mut outbound = Vec::new();
        for effect in effects {
            if let Effect::Send { to, message } = effect {
                self.counter = self
                    .counter
                    .checked_add(1)
                    .ok_or_else(|| error("message counter exhausted"))?;
                for data in encode(self.room, self.writer, self.counter, &message).map_err(error)? {
                    outbound.push(Outbound {
                        to: format!("{to:032x}"),
                        data,
                        control: control(&message),
                    });
                }
            }
        }
        serde_wasm_bindgen::to_value(&Update {
            text: if self.include_text {
                self.text()
            } else {
                String::new()
            },
            history: self.peer.replica().history_len(),
            outbound,
        })
        .map_err(error)
    }
}

#[wasm_bindgen]
pub struct MergeExperiment {
    workload: conv_document::experiment::MergeWorkload,
}
#[wasm_bindgen]
impl MergeExperiment {
    #[wasm_bindgen(constructor)]
    pub fn new(
        visible_bytes: usize,
        samples: usize,
        deletion_heavy: bool,
        seed: u64,
    ) -> Result<MergeExperiment, JsValue> {
        Ok(Self {
            workload: conv_document::experiment::MergeWorkload::new(
                visible_bytes,
                samples,
                deletion_heavy,
                seed,
            )
            .map_err(error)?,
        })
    }
    pub fn merge_one(&mut self) -> Result<bool, JsValue> {
        self.workload.merge_one().map_err(error)
    }
    pub fn verify(&self) -> bool {
        self.workload.verify()
    }
    pub fn tombstones(&self) -> usize {
        self.workload.document().deleted_ids().count()
    }
    pub fn serialized_bytes(&self) -> Result<usize, JsValue> {
        postcard::to_allocvec(self.workload.document())
            .map(|bytes| bytes.len())
            .map_err(error)
    }
}
