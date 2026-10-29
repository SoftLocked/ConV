//! Scheduled in-memory protocol messages. No node reads another node's state.
use conv_document::ElementId;
use conv_protocol::outbox::Outbox;
use conv_protocol::wire::{Reassembler, encode};
use conv_protocol::{
    replication::ReplicationError,
    sync::{Effect, Message, SyncConfig, SyncPeer},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
enum Event {
    Timer(usize),
    MembershipTimer(usize),
    BatchTimer(usize),
    Deliver {
        from: usize,
        to: usize,
        frame: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Faults {
    pub max_delay_ms: u64,
    pub loss_per_thousand: u16,
    pub duplicate_per_thousand: u16,
}

impl Default for Faults {
    fn default() -> Self {
        Self {
            max_delay_ms: 20,
            loss_per_thousand: 0,
            duplicate_per_thousand: 0,
        }
    }
}

#[derive(Debug)]
pub struct SyncNetwork {
    peers: Vec<SyncPeer>,
    blocked: BTreeSet<(usize, usize)>,
    events: BTreeMap<(u64, u64), Event>,
    now: u64,
    serial: u64,
    random: u64,
    transfer: u64,
    incoming: Vec<Reassembler>,
    outboxes: Vec<Outbox>,
    faults: Faults,
    pub dropped: u64,
    pub requests_delivered: u64,
}

impl SyncNetwork {
    pub fn new(count: usize, seed: u64, config: SyncConfig, faults: Faults) -> Self {
        assert!(faults.loss_per_thousand <= 1_000 && faults.duplicate_per_thousand <= 1_000);
        assert!(faults.max_delay_ms < u64::MAX);
        let peers = (0..count)
            .map(|index| {
                SyncPeer::new(
                    index as u128 + 1,
                    (0..count)
                        .filter(|&other| other != index)
                        .map(|other| other as u128 + 1)
                        .collect(),
                    config,
                )
            })
            .collect();
        let mut network = Self {
            peers,
            blocked: BTreeSet::new(),
            events: BTreeMap::new(),
            now: 0,
            serial: 0,
            random: seed,
            transfer: 0,
            outboxes: (0..count).map(|_| Outbox::default()).collect(),
            incoming: (0..count).map(|_| Reassembler::new(1)).collect(),
            faults,
            dropped: 0,
            requests_delivered: 0,
        };
        for index in 0..count {
            network.enqueue(0, Event::Timer(index));
            network.enqueue(0, Event::MembershipTimer(index));
            network.enqueue(20, Event::BatchTimer(index));
        }
        network
    }

    pub fn peer(&self, index: usize) -> &SyncPeer {
        &self.peers[index]
    }

    pub fn partition(&mut self, a: usize, b: usize) {
        self.blocked.insert((a, b));
        self.blocked.insert((b, a));
    }
    pub fn heal(&mut self) {
        self.blocked.clear();
    }

    fn next_random(&mut self) -> u64 {
        self.random = self
            .random
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.random
    }

    fn enqueue(&mut self, delay: u64, event: Event) {
        let at = self.now.checked_add(delay).expect("virtual time overflow");
        self.serial = self.serial.checked_add(1).expect("event ID overflow");
        self.events.insert((at, self.serial), event);
    }

    fn apply(&mut self, from: usize, effects: Vec<Effect>) {
        let immediate = self.outboxes[from].push(effects);
        self.emit(from, immediate);
    }
    fn emit(&mut self, from: usize, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::ScheduleSync { after_ms } => self.enqueue(after_ms, Event::Timer(from)),
                Effect::Send { to, message } => {
                    let to = usize::try_from(to - 1).expect("invalid peer ID");
                    assert!(to < self.peers.len());
                    self.transfer += 1;
                    for frame in encode(1, from as u128 + 1, self.transfer, &message)
                        .expect("simulation message too large")
                    {
                        if self.blocked.contains(&(from, to))
                            || self.next_random() % 1_000 < u64::from(self.faults.loss_per_thousand)
                        {
                            self.dropped += 1;
                            continue;
                        }
                        let delay = self.next_random() % (self.faults.max_delay_ms + 1);
                        if self.next_random() % 1_000
                            < u64::from(self.faults.duplicate_per_thousand)
                        {
                            let duplicate_delay =
                                self.next_random() % (self.faults.max_delay_ms + 1);
                            self.enqueue(
                                duplicate_delay,
                                Event::Deliver {
                                    from,
                                    to,
                                    frame: frame.clone(),
                                },
                            );
                        }
                        self.enqueue(delay, Event::Deliver { from, to, frame });
                    }
                }
            }
        }
    }

    pub fn insert(
        &mut self,
        peer: usize,
        anchor: Option<ElementId>,
        text: &str,
    ) -> Result<(), ReplicationError> {
        let effects = self.peers[peer].insert(anchor, text)?;
        self.apply(peer, effects);
        Ok(())
    }

    pub fn delete(&mut self, peer: usize, ids: &[ElementId]) -> Result<(), ReplicationError> {
        let effects = self.peers[peer].delete(ids)?;
        self.apply(peer, effects);
        Ok(())
    }

    /// Advance virtual time without sleeping. Periodic timers remain scheduled.
    pub fn run_until(&mut self, until: u64) -> Result<(), ReplicationError> {
        assert!(until >= self.now);
        while self
            .events
            .first_key_value()
            .is_some_and(|(&(at, _), _)| at <= until)
        {
            let ((at, _), event) = self.events.pop_first().unwrap();
            self.now = at;
            match event {
                Event::BatchTimer(peer) => {
                    let effects = self.outboxes[peer].flush();
                    self.emit(peer, effects);
                    self.enqueue(20, Event::BatchTimer(peer));
                }
                Event::MembershipTimer(peer) => {
                    let effects = self.peers[peer].on_membership_timer(self.now);
                    self.apply(peer, effects);
                    self.enqueue(100, Event::MembershipTimer(peer));
                }
                Event::Timer(peer) => {
                    let effects = self.peers[peer].on_sync_timer();
                    self.apply(peer, effects);
                }
                Event::Deliver { from, to, frame } => {
                    if self.blocked.contains(&(from, to)) {
                        self.dropped += 1;
                        continue;
                    }
                    if let Ok(Some((sender, message))) = self.incoming[to].receive(self.now, &frame)
                    {
                        assert_eq!(sender, from as u128 + 1);
                        if matches!(&message, Message::Request(_)) {
                            self.requests_delivered += 1;
                        }
                        let effects = self.peers[to].receive_at(sender, message, self.now)?;
                        self.apply(to, effects);
                    }
                }
            }
        }
        self.now = until;
        Ok(())
    }

    pub fn converged(&self) -> bool {
        self.peers.windows(2).all(|pair| {
            pair[0].replica().document() == pair[1].replica().document()
                && pair[0].replica().summary() == pair[1].replica().summary()
        })
    }
}
