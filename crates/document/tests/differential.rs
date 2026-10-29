use conv_document::{Document, Writer, reference::ReferenceDocument};

#[test]
fn indexed_merge_matches_reference_across_seeded_reordered_histories() {
    for seed in 0..100_u64 {
        let mut random = seed;
        let mut writers: Vec<_> = (1..=3)
            .map(|id| Writer::new(id, Document::default()))
            .collect();
        let mut fragments = Vec::new();
        for step in 0..60 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let index = random as usize % writers.len();
            let writer = &mut writers[index];
            let visible = writer.document().visible_elements();
            let delta = if step % 4 == 0 && !visible.is_empty() {
                writer
                    .delete(&[visible[random as usize % visible.len()].0])
                    .unwrap()
            } else {
                let anchor = if random & 8 == 0 {
                    None
                } else {
                    visible.last().map(|&(id, _)| id)
                };
                writer
                    .insert(anchor, ["a", "🦀", "é", "\n"][step % 4])
                    .unwrap()
            };
            fragments.push(delta);
            if step % 3 == 0 {
                let update = fragments[random as usize % fragments.len()].clone();
                writers[(index + 1) % 3].receive(&update).unwrap();
            }
        }
        for index in (1..fragments.len()).rev() {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            fragments.swap(index, random as usize % (index + 1));
        }
        let mut document = Document::default();
        let mut reference = ReferenceDocument::default();
        for fragment in &fragments {
            document.merge(fragment).unwrap();
            reference.merge(fragment).unwrap();
            document.merge(fragment).unwrap();
            reference.merge(fragment).unwrap();
            assert!(reference.matches(&document), "seed {seed}");
            assert_eq!(document.text(), reference.text(), "seed {seed}");
        }
    }
}
