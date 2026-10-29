//! Transport-independent protocol primitives.
//! A mutation summary records exactly which transaction numbers have arrived.
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

pub mod replication;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: u64,
    pub end: u64, // inclusive
}

/// Exact receipt of one writer's mutation numbers. `prefix` means every
/// sequence from 1 through it was received. `sparse` contains later numbers.
/// Zero is never a valid mutation number.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SeenState")]
pub struct MutationSummary {
    prefix: u64,
    sparse: BTreeSet<u64>,
}

#[derive(Deserialize)]
struct SeenState {
    prefix: u64,
    sparse: BTreeSet<u64>,
}
impl TryFrom<SeenState> for MutationSummary {
    type Error = &'static str;
    fn try_from(state: SeenState) -> Result<Self, Self::Error> {
        if state
            .sparse
            .iter()
            .any(|&n| n <= state.prefix || state.prefix.checked_add(1) == Some(n))
        {
            return Err("noncanonical mutation summary");
        }
        Ok(Self {
            prefix: state.prefix,
            sparse: state.sparse,
        })
    }
}

impl MutationSummary {
    pub fn prefix(&self) -> u64 {
        self.prefix
    }
    pub fn out_of_order(&self) -> &BTreeSet<u64> {
        &self.sparse
    }

    pub fn contains(&self, sequence: u64) -> bool {
        sequence != 0 && (sequence <= self.prefix || self.sparse.contains(&sequence))
    }

    /// Returns true only when this is new information.
    pub fn observe(&mut self, sequence: u64) -> Result<bool, &'static str> {
        if sequence == 0 {
            return Err("mutation sequence zero is reserved");
        }
        if self.contains(sequence) {
            return Ok(false);
        }
        if sequence == self.prefix + 1 {
            self.prefix = sequence;
            while let Some(next) = self.prefix.checked_add(1) {
                if self.sparse.remove(&next) {
                    self.prefix = next;
                } else {
                    break;
                }
            }
        } else {
            self.sparse.insert(sequence);
        }
        Ok(true)
    }

    /// Known remote mutations absent locally, grouped into inclusive ranges.
    /// `max_ranges` bounds the output; repeated calls after repair advance it.
    pub fn missing_from(&self, remote: &Self, max_ranges: usize) -> Vec<Range> {
        let mut result = Vec::new();
        if max_ranges == 0 {
            return result;
        }
        if self.prefix < remote.prefix {
            let mut cursor = self.prefix + 1;
            for &known in self.sparse.range(cursor..=remote.prefix) {
                if cursor < known && !push_range(&mut result, cursor, known - 1, max_ranges) {
                    return result;
                }
                if known == u64::MAX {
                    return result;
                }
                cursor = known + 1;
            }
            if cursor <= remote.prefix
                && !push_range(&mut result, cursor, remote.prefix, max_ranges)
            {
                return result;
            }
        }
        for &sequence in &remote.sparse {
            if !self.contains(sequence) && !push_range(&mut result, sequence, sequence, max_ranges)
            {
                break;
            }
        }
        result
    }
}

fn push_range(ranges: &mut Vec<Range>, start: u64, end: u64, max_ranges: usize) -> bool {
    if let Some(last) = ranges.last_mut()
        && last.end.checked_add(1) == Some(start)
    {
        last.end = end;
        return true;
    }
    if ranges.len() == max_ranges {
        return false;
    }
    ranges.push(Range { start, end });
    true
}

pub mod sync;
pub mod wire;

pub mod backup;
pub mod membership;
