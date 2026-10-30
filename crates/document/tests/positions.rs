use conv_document::positions::PositionIndex;
use conv_document::{Document, TextAnchor, Writer};

#[test]
fn indexed_unicode_and_deleted_anchors_match_reference_queries() {
    let mut writer = Writer::new(1, Document::default());
    writer.insert(None, "a🦀é\nend").unwrap();
    let original = writer.document().clone();
    let anchors: Vec<_> = (0..=original.text().encode_utf16().count())
        .filter_map(|offset| original.anchor_utf16(offset))
        .collect();
    for step in 0..8 {
        if let Some((id, _)) = writer.document().visible_elements().get(step % 3).copied() {
            writer.delete(&[id]).unwrap();
        }
        let document = writer.document();
        let index = PositionIndex::new(document);
        assert_eq!(index.text(), document.text());
        for offset in 0..=original.text().encode_utf16().count() {
            let a = index
                .anchor_utf16(offset)
                .map(|anchor| (anchor.left, anchor.right));
            let b = document
                .anchor_utf16(offset)
                .map(|anchor| (anchor.left, anchor.right));
            assert_eq!(a, b);
        }
        for anchor in anchors.iter().copied().chain([TextAnchor {
            left: None,
            right: None,
        }]) {
            assert_eq!(
                index.resolve_anchor(anchor),
                document.resolve_anchor(anchor)
            );
        }
    }
}
