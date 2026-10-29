//! Intentionally simple full-union validation and repeated-scan traversal.
//! Keep independent of the optimized merge path for differential checks.
use crate::{Document, DocumentError, ElementId, Insertion, validate};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub struct ReferenceDocument {
    insertions: BTreeMap<ElementId, Insertion>,
    deletions: BTreeSet<ElementId>,
}
impl ReferenceDocument {
    pub fn merge(&mut self, fragment: &Document) -> Result<(), DocumentError> {
        let mut candidate = self.insertions.clone();
        for record in fragment.insertions() {
            if let Some(existing) = candidate.get(&record.id)
                && existing != record
            {
                return Err(DocumentError::ConflictingInsertion(record.id));
            }
            candidate.insert(record.id, record.clone());
        }
        validate(&candidate)?;
        self.insertions = candidate;
        self.deletions.extend(&fragment.deletions);
        Ok(())
    }
    pub fn matches(&self, document: &Document) -> bool {
        self.insertions == document.insertions && self.deletions == document.deletions
    }
    pub fn text(&self) -> String {
        let mut result = String::new();
        let mut pending = vec![None];
        while let Some(anchor) = pending.pop() {
            if let Some(id) = anchor
                && !self.deletions.contains(&id)
            {
                result.push(self.insertions[&id].value);
            }
            let mut children: Vec<_> = self
                .insertions
                .values()
                .filter(|record| record.anchor == anchor)
                .collect();
            children.sort_by_key(|record| (record.logical_time, record.id));
            pending.extend(children.into_iter().map(|record| Some(record.id)));
        }
        result
    }
}
