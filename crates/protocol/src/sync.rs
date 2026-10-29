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
    Request(
        #[serde(deserialize_with = "conv_document::bounded::vector::<_, _, 64>")] Vec<SyncRequest>,
    ),
    Mutation(Mutation),
    SnapshotOffer {
        mutations: usize,
        bytes: usize,
    },
    SnapshotRequest,
    Snapshot(
        #[serde(deserialize_with = "conv_document::bounded::vector::<_, _, 100_000>")]
        Vec<Mutation>,
    ),
    SummaryReply(CausalSummary),
    Batch(#[serde(deserialize_with = "conv_document::bounded::vector::<_, _, 32>")] Vec<Mutation>),
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
    send_cursor: usize,
    full_mesh_room: bool,
    overlay_room: bool,
}

impl SyncPeer {
    pub fn new(id: u128, neighbors: Vec<u128>, config: SyncConfig) -> Self {
        assert!(
            config.interval_ms > 0
                && config.max_ranges > 0
                && config.max_ranges <= 64
                && config.max_reply_items > 0
        );
        Self::from_replica(Replica::new(id), neighbors, config)
    }

    pub fn from_replica(replica: Replica, neighbors: Vec<u128>, config: SyncConfig) -> Self {
        assert!(
            config.interval_ms > 0
                && config.max_ranges > 0
                && config.max_ranges <= 64
                && config.max_reply_items > 0
        );
        let id = replica.writer_id();
        let mut swim = Swim::new(id);
        swim.set_probe_order(neighbors.clone());
        Self {
            replica,
            neighbors,
            config,
            swim,
            gossip_cursor: 0,
            send_cursor: 0,
            full_mesh_room: false,
            overlay_room: false,
        }
    }
    /// Small browser rooms already have every direct link. Source broadcast
    /// gives bounded one-hop visibility without waiting for probabilistic repair.
    pub fn use_full_mesh_room(&mut self) {
        self.full_mesh_room = true;
        self.overlay_room = false;
    }
    /// Flood each newly learned mutation over bounded reliable overlay links.
    pub fn use_overlay_room(&mut self) {
        self.full_mesh_room = true;
        self.overlay_room = true;
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
    pub fn leave(&mut self) -> Vec<Effect> {
        self.swim
            .leave()
            .into_iter()
            .map(|send| Effect::Send {
                to: send.to,
                message: Message::Swim(send.message),
            })
            .collect()
    }
    pub fn needs_probe_order(&self) -> bool {
        self.swim.needs_probe_order()
    }
    pub fn reshuffle_probes(&mut self, order: Vec<u128>) {
        self.swim.set_probe_order(order);
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

    fn fanout(&mut self, message: Message, exclude: Option<u128>) -> Vec<Effect> {
        if self.neighbors.is_empty() {
            return Vec::new();
        }
        let start = self.send_cursor % self.neighbors.len();
        self.send_cursor = self.send_cursor.wrapping_add(2);
        self.neighbors
            .iter()
            .cycle()
            .skip(start)
            .take(self.neighbors.len())
            .filter(|&&to| Some(to) != exclude)
            .take(if self.overlay_room {
                8
            } else if self.full_mesh_room {
                7
            } else {
                2
            })
            .map(|&to| Effect::Send {
                to,
                message: message.clone(),
            })
            .collect()
    }

    /// Call once to start, then once for each requested sync timer firing.
    pub fn on_sync_timer(&mut self) -> Vec<Effect> {
        let mut effects = self.fanout(Message::Summary(self.replica.summary().clone()), None);
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
            Some(mutation) => self.fanout(Message::Mutation(mutation), None),
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
            .flat_map(|m| self.fanout(Message::Mutation(m), None))
            .collect())
    }

    pub fn delete(&mut self, ids: &[ElementId]) -> Result<Vec<Effect>, ReplicationError> {
        Ok(match self.replica.delete(ids)? {
            Some(mutation) => self.fanout(Message::Mutation(mutation), None),
            None => Vec::new(),
        })
    }

    fn request_missing(&self, from: u128, remote: &CausalSummary) -> Vec<Effect> {
        let requests = self
            .replica
            .summary()
            .missing_from(remote, self.config.max_ranges);
        if requests.is_empty() {
            Vec::new()
        } else {
            vec![Effect::Send {
                to: from,
                message: Message::Request(requests),
            }]
        }
    }
    fn snapshot_message(&self) -> Option<(Message, usize)> {
        if self.replica.history_len() > 100_000 {
            return None;
        }
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
                let mut effects = self.request_missing(from, &remote);
                effects.push(Effect::Send {
                    to: from,
                    message: Message::SummaryReply(self.replica.summary().clone()),
                });
                Ok(effects)
            }
            Message::SummaryReply(remote) => Ok(self.request_missing(from, &remote)),
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
            Message::Batch(mutations) => {
                let mut effects = Vec::new();
                for mutation in mutations {
                    effects.extend(self.receive_at(from, Message::Mutation(mutation), now)?);
                }
                Ok(effects)
            }
            Message::Mutation(mutation) => {
                if self.replica.receive(&mutation)? {
                    if self.full_mesh_room && !self.overlay_room && from != self.replica.writer_id()
                    {
                        return Ok(Vec::new());
                    }
                    Ok(self.fanout(Message::Mutation(mutation), Some(from)))
                } else {
                    Ok(Vec::new())
                }
            }
        }
    }
}
