//! Clarity-first sequence reference model. See docs/crdt-spec.md.
//! No transport, clocks, randomness, persistence, or browser dependencies.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A mutation sequence is contiguous per writer; an offset identifies its elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ElementId {
    pub replica: u128,
    pub sequence: u64,
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insertion {
    pub id: ElementId,
    pub anchor: Option<ElementId>,
    pub logical_time: u64,
    pub value: char,
}

/// Immutable records and grow-only deletion markers form a partial state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DocumentState", into = "DocumentState")]
pub struct Document {
    insertions: BTreeMap<ElementId, Insertion>,
    deletions: BTreeSet<ElementId>,
}

#[derive(Serialize, Deserialize)]
struct DocumentState {
    records: Vec<Insertion>,
    deleted: Vec<ElementId>,
}
impl From<Document> for DocumentState {
    fn from(doc: Document) -> Self {
        Self {
            records: doc.insertions.into_values().collect(),
            deleted: doc.deletions.into_iter().collect(),
        }
    }
}
impl TryFrom<DocumentState> for Document {
    type Error = &'static str;
    fn try_from(state: DocumentState) -> Result<Self, Self::Error> {
        Document::fragment(state.records, state.deleted).map_err(|_| "invalid document state")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentError {
    ConflictingInsertion(ElementId),
    SelfAnchor(ElementId),
    InvalidPredecessorOrder(ElementId),
}

fn validate(records: &BTreeMap<ElementId, Insertion>) -> Result<(), DocumentError> {
    for record in records.values() {
        let Some(anchor) = record.anchor else {
            continue;
        };
        if anchor == record.id {
            return Err(DocumentError::SelfAnchor(record.id));
        }
        let Some(parent) = records.get(&anchor) else {
            continue;
        };
        let earlier_transaction = parent.logical_time < record.logical_time;
        let earlier_offset = parent.logical_time == record.logical_time
            && parent.id.replica == record.id.replica
            && parent.id.sequence == record.id.sequence
            && parent.id.offset < record.id.offset;
        if !earlier_transaction && !earlier_offset {
            return Err(DocumentError::InvalidPredecessorOrder(record.id));
        }
    }
    Ok(())
}

impl Document {
    pub fn insertions(&self) -> impl Iterator<Item = &Insertion> {
        self.insertions.values()
    }

    pub fn is_empty(&self) -> bool {
        self.insertions.is_empty() && self.deletions.is_empty()
    }

    /// Construct a fragment. Missing predecessors remain pending until merged.
    pub fn fragment(
        records: impl IntoIterator<Item = Insertion>,
        deletions: impl IntoIterator<Item = ElementId>,
    ) -> Result<Self, DocumentError> {
        let mut state = Self::default();
        for record in records {
            if let Some(existing) = state.insertions.get(&record.id)
                && existing != &record
            {
                return Err(DocumentError::ConflictingInsertion(record.id));
            }
            state.insertions.insert(record.id, record);
        }
        state.deletions.extend(deletions);
        validate(&state.insertions)?;
        Ok(state)
    }

    /// Join valid states, rejecting conflicting IDs before changing any state.
    pub fn merge(&mut self, other: &Self) -> Result<(), DocumentError> {
        for (id, record) in &other.insertions {
            if let Some(existing) = self.insertions.get(id)
                && existing != record
            {
                return Err(DocumentError::ConflictingInsertion(*id));
            }
        }
        let mut combined = self.insertions.clone();
        combined.extend(other.insertions.clone());
        validate(&combined)?;
        self.insertions = combined;
        self.deletions.extend(&other.deletions);
        Ok(())
    }

    /// Stable element IDs in rendered order, used by editor position adapters.
    pub fn visible_elements(&self) -> Vec<(ElementId, char)> {
        let mut children: BTreeMap<Option<ElementId>, Vec<&Insertion>> = BTreeMap::new();
        for record in self.insertions.values() {
            children.entry(record.anchor).or_default().push(record);
        }
        for siblings in children.values_mut() {
            siblings.sort_by_key(|record| (record.logical_time, record.id));
        }
        let mut output = Vec::new();
        let mut pending = vec![None];
        while let Some(anchor) = pending.pop() {
            if let Some(id) = anchor
                && !self.deletions.contains(&id)
            {
                output.push((id, self.insertions[&id].value));
            }
            if let Some(siblings) = children.get(&anchor) {
                pending.extend(siblings.iter().map(|record| Some(record.id)));
            }
        }
        output
    }

    pub fn text(&self) -> String {
        self.visible_elements()
            .into_iter()
            .map(|(_, value)| value)
            .collect()
    }
}

/// One active writer session. The host must allocate a fresh replica ID; restoring
/// an old writer counter or sharing an ID between tabs is forbidden.
#[derive(Debug)]
pub struct Writer {
    replica: u128,
    sequence: u64,
    clock: u64,
    document: Document,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    UnknownElement(ElementId),
    CounterExhausted,
    InvalidDocument(DocumentError),
}

impl Writer {
    pub fn new(replica: u128, document: Document) -> Self {
        let clock = document
            .insertions
            .values()
            .map(|r| r.logical_time)
            .max()
            .unwrap_or(0);
        Self {
            replica,
            sequence: 0,
            clock,
            document,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn receive(&mut self, fragment: &Document) -> Result<(), EditError> {
        self.document
            .merge(fragment)
            .map_err(EditError::InvalidDocument)?;
        self.clock = self.clock.max(
            fragment
                .insertions
                .values()
                .map(|r| r.logical_time)
                .max()
                .unwrap_or(0),
        );
        Ok(())
    }

    /// Insert Unicode scalars as a single chained transaction and return its delta.
    pub fn insert(&mut self, anchor: Option<ElementId>, text: &str) -> Result<Document, EditError> {
        if let Some(id) = anchor
            && !self.document.insertions.contains_key(&id)
        {
            return Err(EditError::UnknownElement(id));
        }
        if text.is_empty() {
            return Ok(Document::default());
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EditError::CounterExhausted)?;
        let clock = self
            .clock
            .checked_add(1)
            .ok_or(EditError::CounterExhausted)?;
        let mut records = Vec::new();
        let mut predecessor = anchor;
        for (offset, value) in text.chars().enumerate() {
            let offset = u32::try_from(offset).map_err(|_| EditError::CounterExhausted)?;
            let id = ElementId {
                replica: self.replica,
                sequence,
                offset,
            };
            records.push(Insertion {
                id,
                anchor: predecessor,
                logical_time: clock,
                value,
            });
            predecessor = Some(id);
        }
        let delta = Document::fragment(records, []).map_err(EditError::InvalidDocument)?;
        self.document
            .merge(&delta)
            .map_err(EditError::InvalidDocument)?;
        self.sequence = sequence;
        self.clock = clock;
        Ok(delta)
    }

    /// Delete stable IDs, including tombstoned anchors, without deleting children.
    pub fn delete(&mut self, ids: &[ElementId]) -> Result<Document, EditError> {
        for id in ids {
            if !self.document.insertions.contains_key(id) {
                return Err(EditError::UnknownElement(*id));
            }
        }
        if ids.is_empty() {
            return Ok(Document::default());
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EditError::CounterExhausted)?;
        let clock = self
            .clock
            .checked_add(1)
            .ok_or(EditError::CounterExhausted)?;
        let delta =
            Document::fragment([], ids.iter().copied()).map_err(EditError::InvalidDocument)?;
        self.document
            .merge(&delta)
            .map_err(EditError::InvalidDocument)?;
        self.sequence = sequence;
        self.clock = clock;
        Ok(delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(replica: u128, sequence: u64, anchor: Option<ElementId>, value: char) -> Insertion {
        Insertion {
            id: ElementId {
                replica,
                sequence,
                offset: 0,
            },
            anchor,
            logical_time: sequence,
            value,
        }
    }

    #[test]
    fn middle_insertion_precedes_existing_successor() {
        let a = record(1, 1, None, 'a');
        let b = record(1, 2, Some(a.id), 'b');
        let x = record(1, 3, Some(a.id), 'x');
        let state = Document::fragment([a, b, x], []).unwrap();
        assert_eq!(state.text(), "axb");
    }

    #[test]
    fn missing_anchor_and_early_delete_survive_reordering() {
        let a = record(1, 1, None, 'a');
        let child = record(1, 2, Some(a.id), '🦀');
        let mut state = Document::fragment([child], [a.id]).unwrap();
        assert_eq!(state.text(), "");
        state.merge(&Document::fragment([a], []).unwrap()).unwrap();
        assert_eq!(state.text(), "🦀");
    }

    #[test]
    fn two_replicas_converge_with_duplicate_delivery() {
        let left = Document::fragment([record(1, 1, None, 'a')], []).unwrap();
        let right = Document::fragment([record(2, 1, None, 'b')], []).unwrap();
        let mut first = left.clone();
        first.merge(&right).unwrap();
        first.merge(&right).unwrap();
        let mut second = right;
        second.merge(&left).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.text(), "ba");
    }
}
