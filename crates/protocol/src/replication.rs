//! In-memory range repair over immutable transaction fragments.
//! Persistence and transport encoding are host responsibilities still to be added.
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use conv_document::{Document, EditError, ElementId, Writer};

use crate::{MutationSummary, Range};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mutation {
    pub replica: u128,
    pub sequence: u64,
    pub fragment: Document,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplicationError {
    InvalidSequence,
    InvalidMutationFragment,
    InvalidTextRange,
    ConflictingMutation { replica: u128, sequence: u64 },
    Document(EditError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRequest {
    pub replica: u128,
    pub range: Range,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalSummary(BTreeMap<u128, MutationSummary>);

impl CausalSummary {
    pub fn for_replica(&self, replica: u128) -> Option<&MutationSummary> {
        self.0.get(&replica)
    }

    pub fn missing_from(&self, remote: &Self, max_ranges: usize) -> Vec<SyncRequest> {
        let mut requests = Vec::new();
        for (&replica, remote_seen) in &remote.0 {
            let remaining = max_ranges.saturating_sub(requests.len());
            if remaining == 0 {
                break;
            }
            let local_seen = self.0.get(&replica).cloned().unwrap_or_default();
            requests.extend(
                local_seen
                    .missing_from(remote_seen, remaining)
                    .into_iter()
                    .map(|range| SyncRequest { replica, range }),
            );
        }
        requests
    }
}

/// One active writer and retained mutation history. A new instance needs a fresh
/// replica ID supplied by the host. Every accepted mutation remains available
/// for range repair, including mutations whose document delta is deletion-only.
#[derive(Clone, Debug)]
pub struct Replica {
    writer: Writer,
    replica: u128,
    history: BTreeMap<(u128, u64), Mutation>,
    summary: CausalSummary,
}

impl Replica {
    pub fn new(replica: u128) -> Self {
        Self {
            writer: Writer::new(replica, Document::default()),
            replica,
            history: BTreeMap::new(),
            summary: CausalSummary::default(),
        }
    }

    pub fn writer_id(&self) -> u128 {
        self.replica
    }

    pub fn document(&self) -> &Document {
        self.writer.document()
    }
    pub fn summary(&self) -> &CausalSummary {
        &self.summary
    }
    pub(crate) fn history_values(&self) -> impl Iterator<Item = &Mutation> {
        self.history.values()
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn insert(
        &mut self,
        anchor: Option<ElementId>,
        text: &str,
    ) -> Result<Option<Mutation>, ReplicationError> {
        if text.is_empty() {
            return Ok(None);
        }
        let fragment = self
            .writer
            .insert(anchor, text)
            .map_err(ReplicationError::Document)?;
        Ok(Some(self.record_local(fragment)))
    }

    pub fn delete(&mut self, ids: &[ElementId]) -> Result<Option<Mutation>, ReplicationError> {
        if ids.is_empty() {
            return Ok(None);
        }
        let fragment = self
            .writer
            .delete(ids)
            .map_err(ReplicationError::Document)?;
        Ok(Some(self.record_local(fragment)))
    }

    /// Apply one editor replacement. Offsets are UTF-16 code units, matching JS.
    /// Splitting a surrogate pair is rejected before the document changes.
    pub fn edit_utf16(
        &mut self,
        start: usize,
        end: usize,
        text: &str,
    ) -> Result<Vec<Mutation>, ReplicationError> {
        let elements = self.document().visible_elements();
        let mut boundaries = Vec::with_capacity(elements.len() + 1);
        boundaries.push(0);
        for (_, ch) in &elements {
            boundaries.push(boundaries.last().unwrap() + ch.len_utf16());
        }
        let first = boundaries
            .binary_search(&start)
            .map_err(|_| ReplicationError::InvalidTextRange)?;
        let last = boundaries
            .binary_search(&end)
            .map_err(|_| ReplicationError::InvalidTextRange)?;
        if first > last {
            return Err(ReplicationError::InvalidTextRange);
        }
        let anchor = first.checked_sub(1).map(|index| elements[index].0);
        let deleted: Vec<_> = elements[first..last].iter().map(|&(id, _)| id).collect();
        let mut candidate = self.clone();
        let mut mutations = Vec::new();
        if let Some(mutation) = candidate.delete(&deleted)? {
            mutations.push(mutation);
        }
        if let Some(mutation) = candidate.insert(anchor, text)? {
            mutations.push(mutation);
        }
        *self = candidate;
        Ok(mutations)
    }

    fn record_local(&mut self, fragment: Document) -> Mutation {
        let sequence = self.writer.sequence();
        let mutation = Mutation {
            replica: self.replica,
            sequence,
            fragment,
        };
        let key = (self.replica, sequence);
        assert!(
            self.history.insert(key, mutation.clone()).is_none(),
            "writer reused a mutation ID"
        );
        self.summary
            .0
            .entry(self.replica)
            .or_default()
            .observe(sequence)
            .expect("writer generated an invalid sequence");
        mutation
    }

    /// Merge and retain an entire transaction, then mark its number received.
    /// A duplicate transaction must have exactly the same immutable fragment.
    pub fn receive(&mut self, mutation: &Mutation) -> Result<bool, ReplicationError> {
        if mutation.sequence == 0 {
            return Err(ReplicationError::InvalidSequence);
        }
        if mutation.fragment.is_empty()
            || mutation.fragment.insertions().any(|record| {
                record.id.replica != mutation.replica || record.id.sequence != mutation.sequence
            })
        {
            return Err(ReplicationError::InvalidMutationFragment);
        }
        let key = (mutation.replica, mutation.sequence);
        if let Some(existing) = self.history.get(&key) {
            if existing == mutation {
                return Ok(false);
            }
            return Err(ReplicationError::ConflictingMutation {
                replica: mutation.replica,
                sequence: mutation.sequence,
            });
        }
        self.writer
            .receive(&mutation.fragment)
            .map_err(ReplicationError::Document)?;
        self.history.insert(key, mutation.clone());
        self.summary
            .0
            .entry(mutation.replica)
            .or_default()
            .observe(mutation.sequence)
            .expect("nonzero mutation sequence was checked");
        Ok(true)
    }

    /// Serve only retained mutations in the requested range, up to `max_items`.
    pub fn serve(&self, request: SyncRequest, max_items: usize) -> Vec<Mutation> {
        if request.range.start == 0 || request.range.start > request.range.end {
            return Vec::new();
        }
        self.history
            .range((request.replica, request.range.start)..=(request.replica, request.range.end))
            .take(max_items)
            .map(|(_, mutation)| mutation.clone())
            .collect()
    }
}
