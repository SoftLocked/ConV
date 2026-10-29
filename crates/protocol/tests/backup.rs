use conv_document::ElementId;
use conv_protocol::replication::Replica;

#[test]
fn restart_keeps_tombstones_history_and_allocates_a_fresh_writer() {
    let mut original = Replica::new(1);
    original.insert(None, "abc").unwrap();
    original
        .delete(&[ElementId {
            replica: 1,
            sequence: 1,
            offset: 1,
        }])
        .unwrap();
    let bytes = original.backup(10).unwrap();
    let mut restored = Replica::restore(2, 10, &bytes).unwrap();
    assert_eq!(restored.document(), original.document());
    assert_eq!(restored.summary(), original.summary());
    assert!(Replica::restore(1, 10, &bytes).is_err());
    assert!(Replica::restore(2, 11, &bytes).is_err());
    let next = restored.insert(None, "new").unwrap().unwrap();
    assert_eq!((next.replica, next.sequence), (2, 1));
}

#[test]
fn corrupt_or_truncated_backups_do_not_restore() {
    let original = Replica::new(1);
    let mut bytes = original.backup(10).unwrap();
    assert!(Replica::restore(2, 10, &bytes[..bytes.len() - 1]).is_err());
    *bytes.last_mut().unwrap() ^= 1;
    assert!(Replica::restore(2, 10, &bytes).is_err());
}
