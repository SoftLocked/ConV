//! Host-clock SWIM state machine. Hosts supply a shuffled traversal of peers.
//! Failure observations never remove document data.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Status {
    Alive,
    Suspect,
    Dead,
    Left,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: u128,
    pub incarnation: u64,
    pub status: Status,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SwimMessage {
    Ping {
        origin: u128,
        probe: u64,
    },
    Ack {
        origin: u128,
        probe: u64,
        target: u128,
    },
    PingReq {
        origin: u128,
        probe: u64,
        target: u128,
    },
    Update(Member),
}
#[derive(Clone, Debug)]
pub struct Send {
    pub to: u128,
    pub message: SwimMessage,
}
#[derive(Clone, Debug)]
struct Probe {
    target: u128,
    number: u64,
    deadline: u64,
    indirect: bool,
}
#[derive(Clone, Debug)]
pub struct Swim {
    id: u128,
    members: BTreeMap<u128, Member>,
    suspects: BTreeMap<u128, (u64, u64)>,
    relays: BTreeMap<(u128, u64, u128), (u128, u64)>,
    order: Vec<u128>,
    cursor: usize,
    pending: Option<Probe>,
    sequence: u64,
    next_probe: u64,
}
impl Swim {
    pub fn new(id: u128) -> Self {
        Self {
            id,
            members: BTreeMap::from([(
                id,
                Member {
                    id,
                    incarnation: 0,
                    status: Status::Alive,
                },
            )]),
            suspects: BTreeMap::new(),
            relays: BTreeMap::new(),
            order: Vec::new(),
            cursor: 0,
            pending: None,
            sequence: 0,
            next_probe: 0,
        }
    }
    pub fn members(&self) -> &BTreeMap<u128, Member> {
        &self.members
    }
    pub fn set_probe_order(&mut self, order: Vec<u128>) {
        self.order = order.into_iter().filter(|&id| id != self.id).collect();
        self.cursor = 0;
        for &id in &self.order {
            self.members.entry(id).or_insert(Member {
                id,
                incarnation: 0,
                status: Status::Alive,
            });
        }
    }
    fn disseminate(&self, member: Member) -> Vec<Send> {
        self.order
            .iter()
            .cycle()
            .skip(self.cursor % self.order.len().max(1))
            .take(3.min(self.order.len()))
            .map(|&to| Send {
                to,
                message: SwimMessage::Update(member),
            })
            .collect()
    }
    pub fn observe(&mut self, member: Member, now: u64) -> Vec<Send> {
        if member.id == self.id && member.status >= Status::Suspect && member.status != Status::Left
        {
            let own = self.members.get_mut(&self.id).unwrap();
            if member.incarnation >= own.incarnation {
                let Some(next) = member.incarnation.checked_add(1) else {
                    return Vec::new();
                };
                own.incarnation = next;
                own.status = Status::Alive;
                let update = *own;
                return self.disseminate(update);
            }
            return Vec::new();
        }
        if member.id == self.id {
            return Vec::new();
        }
        if let Some(old) = self.members.get(&member.id)
            && (member.incarnation, member.status) <= (old.incarnation, old.status)
        {
            return Vec::new();
        }
        self.members.insert(member.id, member);
        if member.status == Status::Suspect {
            self.suspects
                .insert(member.id, (member.incarnation, now.saturating_add(5_000)));
        } else {
            self.suspects.remove(&member.id);
        }
        self.disseminate(member)
    }
    pub fn leave(&mut self) -> Vec<Send> {
        let own = self.members.get_mut(&self.id).unwrap();
        own.status = Status::Left;
        let update = *own;
        self.disseminate(update)
    }
    pub fn tick(&mut self, now: u64) -> Vec<Send> {
        let mut output = Vec::new();
        self.relays.retain(|_, (_, expires)| *expires > now);
        let expired: Vec<_> = self
            .suspects
            .iter()
            .filter(|(_, (_, until))| *until <= now)
            .map(|(&id, &(incarnation, _))| Member {
                id,
                incarnation,
                status: Status::Dead,
            })
            .collect();
        for member in expired {
            output.extend(self.observe(member, now));
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|probe| probe.deadline <= now)
        {
            let mut probe = self.pending.take().unwrap();
            if !probe.indirect {
                for &helper in self.order.iter().filter(|&&id| id != probe.target).take(3) {
                    output.push(Send {
                        to: helper,
                        message: SwimMessage::PingReq {
                            origin: self.id,
                            probe: probe.number,
                            target: probe.target,
                        },
                    });
                }
                probe.indirect = true;
                probe.deadline = now.saturating_add(250);
                self.pending = Some(probe);
            } else if let Some(member) = self.members.get(&probe.target).copied()
                && member.status == Status::Alive
            {
                output.extend(self.observe(
                    Member {
                        status: Status::Suspect,
                        ..member
                    },
                    now,
                ));
            }
        }
        if self.pending.is_none()
            && now >= self.next_probe
            && self.members[&self.id].status != Status::Left
        {
            for _ in 0..self.order.len() {
                let target = self.order[self.cursor % self.order.len()];
                self.cursor += 1;
                if self
                    .members
                    .get(&target)
                    .is_some_and(|m| m.status < Status::Dead)
                {
                    self.sequence = self
                        .sequence
                        .checked_add(1)
                        .expect("probe counter exhausted");
                    self.next_probe = now.saturating_add(1_000);
                    self.pending = Some(Probe {
                        target,
                        number: self.sequence,
                        deadline: now.saturating_add(250),
                        indirect: false,
                    });
                    output.push(Send {
                        to: target,
                        message: SwimMessage::Ping {
                            origin: self.id,
                            probe: self.sequence,
                        },
                    });
                    break;
                }
            }
        }
        output
    }
    pub fn receive(&mut self, from: u128, message: SwimMessage, now: u64) -> Vec<Send> {
        match message {
            SwimMessage::Ping { origin, probe } => vec![Send {
                to: from,
                message: SwimMessage::Ack {
                    origin,
                    probe,
                    target: self.id,
                },
            }],
            SwimMessage::PingReq {
                origin,
                probe,
                target,
            } => {
                if self.relays.len() >= 256 {
                    return Vec::new();
                }
                self.relays
                    .insert((origin, probe, target), (from, now.saturating_add(1_000)));
                vec![Send {
                    to: target,
                    message: SwimMessage::Ping { origin, probe },
                }]
            }
            SwimMessage::Ack {
                origin,
                probe,
                target,
            } => {
                if origin == self.id
                    && self
                        .pending
                        .as_ref()
                        .is_some_and(|p| p.number == probe && p.target == target)
                {
                    self.pending = None;
                }
                if let Some((to, _)) = self.relays.remove(&(origin, probe, target)) {
                    return vec![Send {
                        to,
                        message: SwimMessage::Ack {
                            origin,
                            probe,
                            target,
                        },
                    }];
                }
                Vec::new()
            }
            SwimMessage::Update(member) => self.observe(member, now),
        }
    }
}
