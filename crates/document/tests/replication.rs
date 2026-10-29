use conv_document::{Document, ElementId, Insertion, Writer};

fn id(replica: u128, sequence: u64, offset: u32) -> ElementId {
    ElementId {
        replica,
        sequence,
        offset,
    }
}

fn join(mut left: Document, right: &Document) -> Document {
    left.merge(right).unwrap();
    left
}

#[test]
fn two_peer_partition_repair_preserves_offline_edits() {
    let mut alice = Writer::new(1, Document::default());
    let initial = alice.insert(None, "ab").unwrap();
    let mut bob = Writer::new(2, Document::default());
    bob.receive(&initial).unwrap();
    let a = id(1, 1, 0);
    let b = id(1, 1, 1);
    let ax = alice.insert(Some(a), "XY").unwrap();
    let bc = bob.insert(Some(a), "🦀e\u{301}\n").unwrap();
    let deletion = alice.delete(&[b]).unwrap();
    // Drop ax and bc, deliver deletion twice, then repair from retained state.
    bob.receive(&deletion).unwrap();
    bob.receive(&deletion).unwrap();
    let alice_snapshot = alice.document().clone();
    let bob_snapshot = bob.document().clone();
    alice.receive(&bob_snapshot).unwrap();
    bob.receive(&alice_snapshot).unwrap();
    assert_eq!(alice.document(), bob.document());
    assert_eq!(alice.document().text(), "a🦀e\u{301}\nXY");
    // Late fast-path messages are harmless after repair.
    alice.receive(&bc).unwrap();
    bob.receive(&ax).unwrap();
    assert_eq!(alice.document(), bob.document());
    // Metadata equality must also support identical future edits.
    let next = alice.insert(Some(b), "!").unwrap();
    bob.receive(&next).unwrap();
    assert_eq!(alice.document(), bob.document());
    assert!(bob.document().text().ends_with('!'));
}

#[test]
fn snapshot_merge_keeps_new_local_work() {
    let mut peer = Writer::new(1, Document::default());
    let snapshot = peer.insert(None, "old").unwrap();
    peer.insert(Some(id(1, 1, 2)), "new").unwrap();
    peer.receive(&snapshot).unwrap();
    assert_eq!(peer.document().text(), "oldnew");
}

#[test]
fn conflicting_record_is_rejected_atomically() {
    let original = Insertion {
        id: id(1, 1, 0),
        anchor: None,
        logical_time: 1,
        value: 'a',
    };
    let mut state = Document::fragment([original.clone()], []).unwrap();
    let before = state.clone();
    let conflict = Insertion {
        value: 'b',
        ..original
    };
    let delta = Document::fragment([conflict], [id(1, 1, 0)]).unwrap();
    assert!(state.merge(&delta).is_err());
    assert_eq!(state, before);
}

#[test]
fn exhaustive_fragment_subsets_obey_merge_laws() {
    let a = Insertion {
        id: id(1, 1, 0),
        anchor: None,
        logical_time: 1,
        value: 'a',
    };
    let b = Insertion {
        id: id(2, 1, 0),
        anchor: Some(a.id),
        logical_time: 2,
        value: 'b',
    };
    let states: Vec<_> = (0..8)
        .map(|bits| {
            Document::fragment(
                [a.clone(), b.clone()]
                    .into_iter()
                    .enumerate()
                    .filter(|(i, _)| bits & (1 << i) != 0)
                    .map(|(_, r)| r),
                if bits & 4 != 0 { vec![a.id] } else { vec![] },
            )
            .unwrap()
        })
        .collect();
    for a in &states {
        assert_eq!(&join(a.clone(), a), a);
        for b in &states {
            assert_eq!(join(a.clone(), b), join(b.clone(), a));
            for c in &states {
                assert_eq!(
                    join(join(a.clone(), b), c),
                    join(a.clone(), &join(b.clone(), c))
                );
            }
        }
    }
}

#[test]
fn every_delivery_order_handles_missing_dependencies_and_deletes() {
    let a = Insertion {
        id: id(1, 1, 0),
        anchor: None,
        logical_time: 1,
        value: 'a',
    };
    let b = Insertion {
        id: id(1, 2, 0),
        anchor: Some(a.id),
        logical_time: 2,
        value: 'b',
    };
    let fragments = [
        Document::fragment([a.clone()], []).unwrap(),
        Document::fragment([b], []).unwrap(),
        Document::fragment([], [a.id]).unwrap(),
    ];
    let expected = fragments.iter().fold(Document::default(), join_ref);
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut state = Document::default();
        for index in order {
            state.merge(&fragments[index]).unwrap();
            state.merge(&fragments[index]).unwrap();
        }
        assert_eq!(state, expected);
        assert_eq!(state.text(), "b");
    }
}

fn join_ref(state: Document, fragment: &Document) -> Document {
    join(state, fragment)
}

#[test]
fn rejects_self_anchor_and_cycle() {
    use conv_document::DocumentError;
    let self_anchor = Insertion {
        id: id(1, 1, 0),
        anchor: Some(id(1, 1, 0)),
        logical_time: 1,
        value: 'a',
    };
    assert_eq!(
        Document::fragment([self_anchor], []).unwrap_err(),
        DocumentError::SelfAnchor(id(1, 1, 0))
    );
    let a = Insertion {
        id: id(1, 1, 0),
        anchor: Some(id(2, 1, 0)),
        logical_time: 1,
        value: 'a',
    };
    let b = Insertion {
        id: id(2, 1, 0),
        anchor: Some(a.id),
        logical_time: 1,
        value: 'b',
    };
    assert!(matches!(
        Document::fragment([a, b], []),
        Err(DocumentError::InvalidPredecessorOrder(_))
    ));
}

#[test]
fn late_predecessor_validation_is_atomic() {
    use conv_document::DocumentError;
    let parent = Insertion {
        id: id(1, 1, 0),
        anchor: None,
        logical_time: 10,
        value: 'p',
    };
    let child = Insertion {
        id: id(2, 1, 0),
        anchor: Some(parent.id),
        logical_time: 9,
        value: 'c',
    };
    let mut state = Document::fragment([child.clone()], []).unwrap();
    let before = state.clone();
    assert_eq!(
        state.merge(&Document::fragment([parent], []).unwrap()),
        Err(DocumentError::InvalidPredecessorOrder(child.id))
    );
    assert_eq!(state, before);
}
