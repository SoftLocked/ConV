use conv_protocol::replication::Replica;

#[test]
fn utf16_replacements_preserve_unicode_and_reject_split_surrogates() {
    let mut replica = Replica::new(1);
    replica.edit_utf16(0, 0, "a🦀e\u{301}\n").unwrap();
    let before = replica.document().clone();
    assert!(replica.edit_utf16(2, 3, "x").is_err());
    assert_eq!(replica.document(), &before);
    replica.edit_utf16(1, 3, "👩‍💻").unwrap();
    assert_eq!(replica.document().text(), "a👩‍💻e\u{301}\n");
    replica.edit_utf16(0, 1, "hello\n").unwrap();
    assert_eq!(replica.document().text(), "hello\n👩‍💻e\u{301}\n");
}
