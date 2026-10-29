//! Host-driven periodic anti-entropy. The host provides timers and transport.
use crate::replication::{CausalSummary, Mutation, Replica, ReplicationError, SyncRequest};
use conv_document::ElementId;

#[derive(Clone, Debug)]
pub enum Message {
    Summary(CausalSummary),
    Request(Vec<SyncRequest>),
    Mutation(Mutation),
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

#[derive(Debug)]
pub struct SyncPeer {
    replica: Replica,
    neighbors: Vec<u128>,
    config: SyncConfig,
}

impl SyncPeer {
    pub fn new(id: u128, neighbors: Vec<u128>, config: SyncConfig) -> Self {
        assert!(config.interval_ms > 0 && config.max_ranges > 0 && config.max_reply_items > 0);
        Self {
            replica: Replica::new(id),
            neighbors,
            config,
        }
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
    pub fn on_sync_timer(&self) -> Vec<Effect> {
        let mut effects = self.broadcast(Message::Summary(self.replica.summary().clone()));
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

    pub fn delete(&mut self, ids: &[ElementId]) -> Result<Vec<Effect>, ReplicationError> {
        Ok(match self.replica.delete(ids)? {
            Some(mutation) => self.broadcast(Message::Mutation(mutation)),
            None => Vec::new(),
        })
    }

    pub fn receive(
        &mut self,
        from: u128,
        message: Message,
    ) -> Result<Vec<Effect>, ReplicationError> {
        match message {
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
