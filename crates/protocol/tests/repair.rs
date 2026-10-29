use conv_document::{Document, ElementId, Insertion};
use conv_protocol::replication::{Mutation, Replica, ReplicationError};

fn exchange(source: &Replica, target: &mut Replica, max_ranges: usize, max_items: usize) {
    let requests = target.summary().missing_from(source.summary(), max_ranges);
    for request in requests {
        for mutation in source.serve(request, max_items) {
            target.receive(&mutation).unwrap();
        }
    }
}

#[test]
fn repairs_missing_insert_and_delete_transactions() {
    let mut alice = Replica::new(1);
    let mut bob = Replica::new(2);
    let first = alice.insert(None, "ab").unwrap().unwrap();
    bob.receive(&first).unwrap();
    let a = ElementId {
        replica: 1,
        sequence: 1,
        offset: 0,
    };
    let b = ElementId {
        replica: 1,
        sequence: 1,
        offset: 1,
    };
    let missing = alice.insert(Some(a), "x").unwrap().unwrap();
    let deletion = alice.delete(&[b]).unwrap().unwrap();
    bob.receive(&deletion).unwrap(); // Arrival with a gap is allowed.
    bob.insert(Some(a), "🦀").unwrap();
    assert_eq!(bob.summary().for_replica(1).unwrap().prefix(), 1);
    assert!(!bob.receive(&deletion).unwrap());
    let requests = bob.summary().missing_from(alice.summary(), 10);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].range.start, 2);
    assert_eq!(requests[0].range.end, 2);
    exchange(&alice, &mut bob, 10, 10);
    exchange(&bob, &mut alice, 10, 10);
    assert_eq!(alice.document(), bob.document());
    assert_eq!(alice.history_len(), 4);
    assert_eq!(bob.history_len(), 4);
    assert!(!bob.receive(&missing).unwrap());
}

#[test]
fn bounded_repair_advances_until_caught_up() {
    let mut source = Replica::new(1);
    let mut target = Replica::new(2);
    for i in 0..20 {
        let mutation = source.insert(None, "x").unwrap().unwrap();
        if i % 3 == 0 {
            target.receive(&mutation).unwrap();
        }
    }
    for _ in 0..20 {
        exchange(&source, &mut target, 1, 2);
    }
    assert_eq!(target.document(), source.document());
    assert_eq!(target.summary(), source.summary());
}

#[test]
fn conflicting_duplicate_does_not_change_document_or_receipt() {
    let mut source = Replica::new(1);
    let first = source.insert(None, "x").unwrap().unwrap();
    let mut target = Replica::new(2);
    target.receive(&first).unwrap();
    let before = target.document().clone();
    let summary = target.summary().clone();
    let forged = Mutation {
        fragment: Document::fragment(
            [Insertion {
                id: ElementId {
                    replica: 1,
                    sequence: 1,
                    offset: 0,
                },
                anchor: None,
                logical_time: 1,
                value: 'y',
            }],
            [],
        )
        .unwrap(),
        ..first
    };
    assert!(matches!(
        target.receive(&forged),
        Err(ReplicationError::ConflictingMutation { .. })
    ));
    assert_eq!(target.document(), &before);
    assert_eq!(target.summary(), &summary);
}

#[test]
fn rejects_mismatched_mutation_identity_before_receipt() {
    let mut source = Replica::new(1);
    let first = source.insert(None, "x").unwrap().unwrap();
    let mut target = Replica::new(2);
    let mislabeled = Mutation {
        replica: 3,
        ..first
    };
    assert_eq!(
        target.receive(&mislabeled),
        Err(ReplicationError::InvalidMutationFragment)
    );
    assert_eq!(target.history_len(), 0);
    assert!(target.summary().for_replica(3).is_none());
}
