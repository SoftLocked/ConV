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

#[test]
fn snapshot_bootstrap_preserves_edits_made_while_it_arrives() {
    let mut source = SyncPeer::new(1, vec![2], SyncConfig::default());
    for _ in 0..50 {
        source.insert(None, "x").unwrap();
    }
    let mut target = SyncPeer::new(2, vec![1], SyncConfig::default());
    let offer = source
        .receive(2, Message::Hello)
        .unwrap()
        .into_iter()
        .find_map(|effect| match effect {
            Effect::Send {
                message: message @ Message::SnapshotOffer { .. },
                ..
            } => Some(message),
            _ => None,
        })
        .unwrap();
    let request = target.receive(1, offer).unwrap().remove(0);
    let Effect::Send { message, .. } = request else {
        panic!("expected request")
    };
    let reply = source.receive(2, message).unwrap().remove(0);
    target.insert(None, "local").unwrap();
    let Effect::Send { message, .. } = reply else {
        panic!("expected snapshot")
    };
    target.receive(1, message).unwrap();
    assert_eq!(target.replica().history_len(), 51);
    assert!(target.replica().document().text().contains("local"));
    assert_eq!(
        target
            .replica()
            .document()
            .text()
            .chars()
            .filter(|&ch| ch == 'x')
            .count(),
        50
    );
}
