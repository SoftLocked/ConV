//! Shared deterministic workload for native/WASM merge measurements.
//! Timing is supplied by the host and is intentionally absent from this module.
use crate::{Document, DocumentError, EditError, Writer};
pub struct MergeWorkload {
    document: Document,
    expected: Document,
    deltas: Vec<Document>,
    next: usize,
}
impl MergeWorkload {
    pub fn new(
        visible_bytes: usize,
        samples: usize,
        deletion_heavy: bool,
        seed: u64,
    ) -> Result<Self, EditError> {
        if visible_bytes == 0 || visible_bytes > 1_048_576 || samples > 100_000 {
            return Err(EditError::TransactionTooLarge);
        }
        let mut writer = Writer::new(1, Document::default());
        let mut anchor = None;
        let mut remaining = visible_bytes * if deletion_heavy { 2 } else { 1 };
        while remaining > 0 {
            let count = remaining.min(4_096);
            let fragment = writer.insert(anchor, &"x".repeat(count))?;
            anchor = fragment.insertions().last().map(|record| record.id);
            remaining -= count;
        }
        if deletion_heavy {
            let ids: Vec<_> = writer
                .document()
                .visible_elements()
                .into_iter()
                .enumerate()
                .filter(|(index, _)| index % 2 == 0)
                .map(|(_, (id, _))| id)
                .collect();
            for chunk in ids.chunks(4_096) {
                writer.delete(chunk)?;
            }
        }
        let document = writer.document().clone();
        let anchors = document.visible_elements();
        let mut random = seed;
        let mut deltas = Vec::new();
        for _ in 0..samples {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let anchor = if random & 3 == 0 {
                None
            } else {
                Some(anchors[random as usize % anchors.len()].0)
            };
            deltas.push(writer.insert(anchor, "z")?);
        }
        Ok(Self {
            document,
            expected: writer.document,
            deltas,
            next: 0,
        })
    }
    pub fn merge_one(&mut self) -> Result<bool, DocumentError> {
        let Some(delta) = self.deltas.get(self.next) else {
            return Ok(false);
        };
        self.document.merge(delta)?;
        self.next += 1;
        Ok(true)
    }
    pub fn verify(&self) -> bool {
        self.next == self.deltas.len() && self.document == self.expected
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
}
