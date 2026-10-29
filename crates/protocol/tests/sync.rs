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

#[test]
fn each_node_forwards_new_information_once_with_bounded_fanout() {
    let mut source = SyncPeer::new(1, vec![2, 3, 4, 5], SyncConfig::default());
    let outgoing = source.insert(None, "x").unwrap();
    assert_eq!(outgoing.len(), 2);
    let Effect::Send { message, .. } = outgoing[0].clone() else {
        panic!("expected delta")
    };
    let mut receiver = SyncPeer::new(2, vec![1, 3, 4, 5], SyncConfig::default());
    let forwards = receiver.receive(1, message.clone()).unwrap();
    assert_eq!(forwards.len(), 2);
    assert!(
        forwards
            .iter()
            .all(|effect| matches!(effect, Effect::Send { to, .. } if *to != 1))
    );
    assert!(receiver.receive(1, message).unwrap().is_empty());
}

#[test]
fn full_mesh_browser_mode_broadcasts_only_from_the_source() {
    let mut source = SyncPeer::new(1, vec![2, 3, 4, 5, 6, 7, 8], SyncConfig::default());
    source.use_full_mesh_room();
    let outgoing = source.insert(None, "x").unwrap();
    assert_eq!(outgoing.len(), 7);
    let Effect::Send { message, .. } = outgoing[0].clone() else {
        panic!("expected mutation")
    };
    let mut receiver = SyncPeer::new(2, vec![1, 3, 4, 5, 6, 7, 8], SyncConfig::default());
    receiver.use_full_mesh_room();
    assert!(receiver.receive(1, message).unwrap().is_empty());
    assert_eq!(receiver.replica().document().text(), "x");
}

#[test]
fn reliable_overlay_relays_new_mutations_and_suppresses_loops() {
    let mut source = SyncPeer::new(1, vec![2], SyncConfig::default());
    let mut relay = SyncPeer::new(2, vec![1, 3], SyncConfig::default());
    let mut target = SyncPeer::new(3, vec![2], SyncConfig::default());
    for peer in [&mut source, &mut relay, &mut target] {
        peer.use_overlay_room();
    }
    let Effect::Send { message, .. } = source.insert(None, "shared").unwrap().remove(0) else {
        panic!("expected send");
    };
    let sends = relay.receive(1, message.clone()).unwrap();
    assert!(relay.receive(1, message).unwrap().is_empty());
    assert_eq!(sends.len(), 1);
    let Effect::Send { to: 3, message } = sends[0].clone() else {
        panic!("expected relay");
    };
    target.receive(2, message).unwrap();
    assert_eq!(source.replica().document(), target.replica().document());
    assert_eq!(source.replica().summary(), target.replica().summary());
}
