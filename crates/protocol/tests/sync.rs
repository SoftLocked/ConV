use conv_protocol::{
    Range,
    replication::SyncRequest,
    sync::{Effect, Message, SyncConfig, SyncPeer},
};

#[test]
fn reply_budget_is_shared_across_all_requested_ranges() {
    let mut peer = SyncPeer::new(
        1,
        vec![2],
        SyncConfig {
            interval_ms: 100,
            max_ranges: 2,
            max_reply_items: 3,
        },
    );
    for _ in 0..10 {
        peer.insert(None, "x").unwrap();
    }
    let requests = vec![
        SyncRequest {
            replica: 1,
            range: Range { start: 1, end: 2 },
        },
        SyncRequest {
            replica: 1,
            range: Range { start: 3, end: 10 },
        },
    ];
    let replies = peer.receive(2, Message::Request(requests)).unwrap();
    assert_eq!(replies.len(), 3);
    for (i, reply) in replies.into_iter().enumerate() {
        let Effect::Send {
            to: 2,
            message: Message::Mutation(mutation),
        } = reply
        else {
            panic!("unexpected effect")
        };
        assert_eq!(mutation.sequence, i as u64 + 1);
    }
}
