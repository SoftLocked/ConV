//! Transport-independent protocol primitives.
//! A mutation summary records exactly which transaction numbers have arrived.

use std::collections::BTreeSet;

pub mod replication;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: u64,
    pub end: u64, // inclusive
}

/// Exact receipt of one writer's mutation numbers. `prefix` means every
/// sequence from 1 through it was received. `sparse` contains later numbers.
/// Zero is never a valid mutation number.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MutationSummary {
    prefix: u64,
    sparse: BTreeSet<u64>,
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
        // The prefix scan is deliberately simple. A later indexed range set can
        // avoid scanning large histories while preserving these exact semantics.
        for sequence in 1..=remote.prefix {
            if !self.contains(sequence) && !push_missing(&mut result, sequence, max_ranges) {
                return result;
            }
        }
        for &sequence in &remote.sparse {
            if !self.contains(sequence) && !push_missing(&mut result, sequence, max_ranges) {
                break;
            }
        }
        result
    }
}

fn push_missing(ranges: &mut Vec<Range>, sequence: u64, max_ranges: usize) -> bool {
    if let Some(last) = ranges.last_mut()
        && last.end.checked_add(1) == Some(sequence)
    {
        last.end = sequence;
        return true;
    }
    if ranges.len() == max_ranges {
        return false;
    }
    ranges.push(Range {
        start: sequence,
        end: sequence,
    });
    true
}

pub mod sync;
