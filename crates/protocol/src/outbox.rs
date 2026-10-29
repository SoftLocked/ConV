//! Bounded immediate-gossip queue. Dropping here never discards retained history.
use crate::{
    replication::Mutation,
    sync::{Effect, Message},
};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Outbox {
    pending: BTreeMap<u128, Vec<Mutation>>,
    bytes: usize,
    count: usize,
}

pub fn control(message: &Message) -> bool {
    !matches!(
        message,
        Message::Mutation(_) | Message::Batch(_) | Message::Snapshot(_)
    )
}

impl Outbox {
    pub fn push(&mut self, effects: Vec<Effect>) -> Vec<Effect> {
        let mut immediate = Vec::new();
        for effect in effects {
            match effect {
                Effect::Send {
                    to,
                    message: Message::Mutation(mutation),
                } => {
                    if self.pending.get(&to).is_some_and(|items| {
                        items.iter().any(|item| {
                            item.replica == mutation.replica && item.sequence == mutation.sequence
                        })
                    }) {
                        continue;
                    }
                    let size = postcard::to_allocvec(&mutation)
                        .map(|bytes| bytes.len())
                        .unwrap_or(usize::MAX);
                    if self.count >= 256 || size > 8 * 1024 * 1024 - self.bytes {
                        continue;
                    }
                    self.bytes += size;
                    self.count += 1;
                    self.pending.entry(to).or_default().push(mutation);
                }
                other => immediate.push(other),
            }
        }
        immediate
    }
    /// The host calls this every 20 ms. Small mutations share a frame; larger
    /// mutations remain individually chunked by the wire encoder.
    pub fn flush(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        for (to, mutations) in std::mem::take(&mut self.pending) {
            let mut batch = Vec::new();
            let mut size = 0;
            for mutation in mutations {
                let bytes = postcard::to_allocvec(&mutation)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX);
                if !batch.is_empty() && (batch.len() >= 32 || size + bytes > 900) {
                    effects.push(send(to, std::mem::take(&mut batch)));
                    size = 0;
                }
                size += bytes;
                batch.push(mutation);
            }
            if !batch.is_empty() {
                effects.push(send(to, batch));
            }
        }
        self.bytes = 0;
        self.count = 0;
        effects
    }
}
fn send(to: u128, mut batch: Vec<Mutation>) -> Effect {
    let message = if batch.len() == 1 {
        Message::Mutation(batch.pop().unwrap())
    } else {
        Message::Batch(batch)
    };
    Effect::Send { to, message }
}
