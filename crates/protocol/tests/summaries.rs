use conv_protocol::{MutationSummary, Range};

#[test]
fn a_high_sequence_does_not_acknowledge_holes() {
    let mut seen = MutationSummary::default();
    seen.observe(5).unwrap();
    assert_eq!(seen.prefix(), 0);
    assert!(!seen.contains(4));
    for sequence in [1, 3, 2, 4] {
        seen.observe(sequence).unwrap();
    }
    assert_eq!(seen.prefix(), 5);
    assert!(seen.out_of_order().is_empty());
    assert!(!seen.observe(5).unwrap());
    assert!(seen.observe(0).is_err());
}

#[test]
fn request_only_remote_mutations_that_are_missing() {
    let mut local = MutationSummary::default();
    let mut remote = MutationSummary::default();
    for sequence in [1, 2, 4, 7] {
        local.observe(sequence).unwrap();
    }
    for sequence in [1, 2, 3, 4, 5, 6, 8] {
        remote.observe(sequence).unwrap();
    }
    assert_eq!(
        local.missing_from(&remote, 10),
        vec![
            Range { start: 3, end: 3 },
            Range { start: 5, end: 6 },
            Range { start: 8, end: 8 },
        ]
    );
    assert_eq!(
        local.missing_from(&remote, 2),
        vec![Range { start: 3, end: 3 }, Range { start: 5, end: 6 },]
    );
}
