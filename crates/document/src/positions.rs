//! A derived view for repeated cursor queries between immutable-history changes.
use crate::{Document, ElementId, TextAnchor};
use std::collections::BTreeMap;

pub struct PositionIndex {
    text: String,
    boundaries: Vec<usize>,
    visible: Vec<ElementId>,
    offsets: BTreeMap<ElementId, (usize, usize)>,
}
impl PositionIndex {
    pub fn new(document: &Document) -> Self {
        let mut index = Self {
            text: String::new(),
            boundaries: vec![0],
            visible: Vec::new(),
            offsets: BTreeMap::new(),
        };
        let mut offset = 0;
        for (id, value) in document.ordered_elements() {
            let before = offset;
            if !document.deletions.contains(&id) {
                index.text.push(value);
                index.visible.push(id);
                offset += value.len_utf16();
                index.boundaries.push(offset);
            }
            index.offsets.insert(id, (before, offset));
        }
        index
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn anchor_utf16(&self, position: usize) -> Option<TextAnchor> {
        let index = self.boundaries.binary_search(&position).ok()?;
        Some(TextAnchor {
            left: index.checked_sub(1).map(|i| self.visible[i]),
            right: self.visible.get(index).copied(),
        })
    }
    pub fn resolve_anchor(&self, anchor: TextAnchor) -> usize {
        if let Some(offset) = anchor.right.and_then(|id| self.offsets.get(&id)) {
            return offset.0;
        }
        anchor
            .left
            .and_then(|id| self.offsets.get(&id))
            .map_or(0, |offset| offset.1)
    }
}
