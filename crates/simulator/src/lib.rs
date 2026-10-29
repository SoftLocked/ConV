//! Deterministic document delivery harness. Messages are in-memory fragments;
//! encoded protocol messages, periodic repair, SWIM, and churn are future work.
use conv_document::{Document, EditError, Writer};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct Delivery {
    from: usize,
    to: usize,
    fragment: Document,
}

/// Virtual milliseconds and a monotonic tie breaker define reproducible delivery.
/// Partitions drop traffic both when sent and when an in-flight message arrives.
#[derive(Debug)]
pub struct Network {
    peers: Vec<Writer>,
    blocked: BTreeSet<(usize, usize)>,
    queue: BTreeMap<(u64, u64), Delivery>,
    now: u64,
    serial: u64,
}

impl Network {
    pub fn new(count: usize) -> Self {
        Self {
            peers: (0..count)
                .map(|i| Writer::new(i as u128 + 1, Document::default()))
                .collect(),
            blocked: BTreeSet::new(),
            queue: BTreeMap::new(),
            now: 0,
            serial: 0,
        }
    }

    pub fn peer(&self, index: usize) -> &Writer {
        &self.peers[index]
    }
    pub fn peer_mut(&mut self, index: usize) -> &mut Writer {
        &mut self.peers[index]
    }
    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn partition(&mut self, first: usize, second: usize) {
        self.blocked.insert((first, second));
        self.blocked.insert((second, first));
    }

    pub fn heal(&mut self) {
        self.blocked.clear();
    }

    /// Call more than once to model duplication; omit a send to model loss.
    pub fn send(&mut self, from: usize, to: usize, fragment: Document, delay: u64) {
        assert!(from < self.peers.len() && to < self.peers.len());
        if self.blocked.contains(&(from, to)) {
            return;
        }
        let at = self.now.checked_add(delay).expect("virtual time overflow");
        self.serial = self.serial.checked_add(1).expect("event counter overflow");
        self.queue
            .insert((at, self.serial), Delivery { from, to, fragment });
    }

    /// Explicit full-state repair baseline. Not the eventual range-repair protocol.
    pub fn repair(&mut self, from: usize, to: usize, delay: u64) {
        self.send(from, to, self.peers[from].document().clone(), delay);
    }

    pub fn drain(&mut self) -> Result<(), EditError> {
        while let Some(((at, _), message)) = self.queue.pop_first() {
            self.now = at;
            if !self.blocked.contains(&(message.from, message.to)) {
                self.peers[message.to].receive(&message.fragment)?;
            }
        }
        Ok(())
    }

    pub fn converged(&self) -> bool {
        self.peers
            .windows(2)
            .all(|pair| pair[0].document() == pair[1].document())
    }
}
