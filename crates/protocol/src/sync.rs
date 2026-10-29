//! Host-driven periodic anti-entropy. The host provides timers and transport.
use crate::membership::{Swim, SwimMessage};
use crate::replication::{CausalSummary, Mutation, Replica, ReplicationError, SyncRequest};
use conv_document::ElementId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    Hello,
    Swim(SwimMessage),
    Summary(CausalSummary),
    Request(Vec<SyncRequest>),
    Mutation(Mutation),
    SnapshotOffer { mutations: usize, bytes: usize },
    SnapshotRequest,
    Snapshot(Vec<Mutation>),
}

#[derive(Clone, Debug)]
pub enum Effect {
    Send { to: u128, message: Message },
    ScheduleSync { after_ms: u64 },
}

#[derive(Clone, Copy, Debug)]
pub struct SyncConfig {
    pub interval_ms: u64,
    pub max_ranges: usize,
    pub max_reply_items: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1_000,
            max_ranges: 16,
            max_reply_items: 32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyncPeer {
    replica: Replica,
    neighbors: Vec<u128>,
    config: SyncConfig,
    swim: Swim,
    gossip_cursor: usize,
}

impl SyncPeer {
    pub fn new(id: u128, neighbors: Vec<u128>, config: SyncConfig) -> Self {
        assert!(config.interval_ms > 0 && config.max_ranges > 0 && config.max_reply_items > 0);
        Self::from_replica(Replica::new(id), neighbors, config)
    }

    pub fn from_replica(replica: Replica, neighbors: Vec<u128>, config: SyncConfig) -> Self {
        assert!(config.interval_ms > 0 && config.max_ranges > 0 && config.max_reply_items > 0);
        let id = replica.writer_id();
        let mut swim = Swim::new(id);
        swim.set_probe_order(neighbors.clone());
        Self {
            replica,
            neighbors,
            config,
            swim,
            gossip_cursor: 0,
        }
    }
    pub fn merge_backup(
        &mut self,
        room: u128,
        bytes: &[u8],
    ) -> Result<(), crate::backup::BackupError> {
        self.replica.merge_backup(room, bytes)
    }
    pub fn set_neighbors(&mut self, neighbors: Vec<u128>) {
        if self.neighbors != neighbors {
            self.swim.set_probe_order(neighbors.clone());
        }
        self.neighbors = neighbors;
    }
    pub fn membership(&self) -> &Swim {
        &self.swim
    }
    pub fn on_membership_timer(&mut self, now: u64) -> Vec<Effect> {
        self.swim
            .tick(now)
            .into_iter()
            .map(|send| Effect::Send {
                to: send.to,
                message: Message::Swim(send.message),
            })
            .collect()
    }

    pub fn replica(&self) -> &Replica {
        &self.replica
    }

    fn broadcast(&self, message: Message) -> Vec<Effect> {
        self.neighbors
            .iter()
            .map(|&to| Effect::Send {
                to,
                message: message.clone(),
            })
            .collect()
    }

    /// Call once to start, then once for each requested sync timer firing.
    pub fn on_sync_timer(&mut self) -> Vec<Effect> {
        let mut effects = self.broadcast(Message::Summary(self.replica.summary().clone()));
        let observations: Vec<_> = self.swim.members().values().copied().collect();
        if !self.neighbors.is_empty() && !observations.is_empty() {
            for member in observations
                .iter()
                .cycle()
                .skip(self.gossip_cursor % observations.len())
                .take(3.min(observations.len()))
            {
                for &to in self
                    .neighbors
                    .iter()
                    .cycle()
                    .skip(self.gossip_cursor % self.neighbors.len())
                    .take(3.min(self.neighbors.len()))
                {
                    effects.push(Effect::Send {
                        to,
                        message: Message::Swim(SwimMessage::Update(*member)),
                    });
                }
            }
            self.gossip_cursor = self.gossip_cursor.wrapping_add(3);
        }
        effects.push(Effect::ScheduleSync {
            after_ms: self.config.interval_ms,
        });
        effects
    }

    pub fn insert(
        &mut self,
        anchor: Option<ElementId>,
        text: &str,
    ) -> Result<Vec<Effect>, ReplicationError> {
        Ok(match self.replica.insert(anchor, text)? {
            Some(mutation) => self.broadcast(Message::Mutation(mutation)),
            None => Vec::new(),
        })
    }

    pub fn edit_utf16(
        &mut self,
        start: usize,
        end: usize,
        text: &str,
    ) -> Result<Vec<Effect>, ReplicationError> {
        let mutations = self.replica.edit_utf16(start, end, text)?;
        Ok(mutations
            .into_iter()
            .flat_map(|m| self.broadcast(Message::Mutation(m)))
            .collect())
    }

    pub fn delete(&mut self, ids: &[ElementId]) -> Result<Vec<Effect>, ReplicationError> {
        Ok(match self.replica.delete(ids)? {
            Some(mutation) => self.broadcast(Message::Mutation(mutation)),
            None => Vec::new(),
        })
    }

    fn snapshot_message(&self) -> Option<(Message, usize)> {
        let message = Message::Snapshot(self.replica.snapshot());
        let size = postcard::to_allocvec(&message).ok()?.len();
        (size < crate::wire::MESSAGE_LIMIT - 16).then_some((message, size))
    }
    pub fn receive(
        &mut self,
        from: u128,
        message: Message,
    ) -> Result<Vec<Effect>, ReplicationError> {
        self.receive_at(from, message, 0)
    }
    pub fn receive_at(
        &mut self,
        from: u128,
        message: Message,
        now: u64,
    ) -> Result<Vec<Effect>, ReplicationError> {
        match message {
            Message::Swim(message) => Ok(self
                .swim
                .receive(from, message, now)
                .into_iter()
                .map(|send| Effect::Send {
                    to: send.to,
                    message: Message::Swim(send.message),
                })
                .collect()),
            Message::Hello => {
                let mut replies = vec![Effect::Send {
                    to: from,
                    message: Message::Summary(self.replica.summary().clone()),
                }];
                if self.replica.history_len() > self.config.max_reply_items
                    && let Some((_, bytes)) = self.snapshot_message()
                {
                    replies.push(Effect::Send {
                        to: from,
                        message: Message::SnapshotOffer {
                            mutations: self.replica.history_len(),
                            bytes,
                        },
                    });
                }
                Ok(replies)
            }
            Message::SnapshotOffer { mutations, bytes } => {
                if mutations
                    > self
                        .replica
                        .history_len()
                        .saturating_add(self.config.max_reply_items)
                    && bytes <= crate::wire::MESSAGE_LIMIT
                    && mutations <= 100_000
                {
                    Ok(vec![Effect::Send {
                        to: from,
                        message: Message::SnapshotRequest,
                    }])
                } else {
                    Ok(Vec::new())
                }
            }
            Message::SnapshotRequest => Ok(self
                .snapshot_message()
                .map(|(message, _)| vec![Effect::Send { to: from, message }])
                .unwrap_or_default()),
            Message::Snapshot(mutations) => {
                if mutations.len() > 100_000 {
                    return Err(ReplicationError::InvalidMutationFragment);
                }
                self.replica.merge_snapshot(&mutations)?;
                Ok(Vec::new())
            }
            Message::Summary(remote) => {
                let requests = self
                    .replica
                    .summary()
                    .missing_from(&remote, self.config.max_ranges);
                if requests.is_empty() {
                    return Ok(Vec::new());
                }
                Ok(vec![Effect::Send {
                    to: from,
                    message: Message::Request(requests),
                }])
            }
            Message::Request(requests) => {
                let mut effects = Vec::new();
                for request in requests.into_iter().take(self.config.max_ranges) {
                    for mutation in self
                        .replica
                        .serve(request, self.config.max_reply_items - effects.len())
                    {
                        effects.push(Effect::Send {
                            to: from,
                            message: Message::Mutation(mutation),
                        });
                    }
                    if effects.len() == self.config.max_reply_items {
                        break;
                    }
                }
                Ok(effects)
            }
            Message::Mutation(mutation) => {
                self.replica.receive(&mutation)?;
                Ok(Vec::new())
            }
        }
    }
}
