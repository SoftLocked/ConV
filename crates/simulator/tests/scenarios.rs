use conv_document::ElementId;
use conv_simulator::Network;

#[test]
fn three_way_partition_edits_and_repair() {
    let mut net = Network::new(3);
    for a in 0..3 {
        for b in a + 1..3 {
            net.partition(a, b);
        }
    }
    for peer in 0..3 {
        let delta = net
            .peer_mut(peer)
            .insert(None, ["alpha", "βeta", "🦀"][peer])
            .unwrap();
        for target in 0..3 {
            net.send(peer, target, delta.clone(), 10);
        }
    }
    net.drain().unwrap();
    assert!(!net.converged());
    net.heal();
    for source in 0..3 {
        for target in 0..3 {
            net.repair(source, target, 10);
        }
    }
    net.drain().unwrap();
    assert!(net.converged());
    assert_eq!(net.peer(0).document().text(), "🦀βetaalpha");
    assert_eq!(net.now(), 20);
}

#[test]
fn seeded_reordering_duplication_loss_then_repair() {
    for seed in 1..=100_u64 {
        let mut random = seed;
        let mut net = Network::new(2);
        for sequence in 1..=30 {
            let anchor = (sequence > 1).then(|| ElementId {
                replica: 1,
                sequence: sequence - 1,
                offset: 0,
            });
            let delta = net.peer_mut(0).insert(anchor, "x").unwrap();
            // Fixed LCG for test scheduling only; never used for replica identity.
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            if random % 5 != 0 {
                net.send(0, 1, delta.clone(), random % 100);
                net.send(0, 1, delta, random % 37);
            }
        }
        net.drain().unwrap();
        net.repair(0, 1, 1);
        net.drain().unwrap();
        assert!(net.converged(), "seed {seed}");
        assert_eq!(net.peer(1).document().text(), "x".repeat(30), "seed {seed}");
    }
}

#[test]
fn bootstrap_during_editing_keeps_local_and_late_work() {
    let mut net = Network::new(2);
    net.peer_mut(0).insert(None, "initial").unwrap();
    net.repair(0, 1, 50);
    let late = net
        .peer_mut(0)
        .insert(
            Some(ElementId {
                replica: 1,
                sequence: 1,
                offset: 6,
            }),
            "!",
        )
        .unwrap();
    net.peer_mut(1).insert(None, "local").unwrap();
    net.send(0, 1, late, 1); // Child arrives before its snapshot dependency.
    net.drain().unwrap();
    net.repair(1, 0, 1);
    net.drain().unwrap();
    assert!(net.converged());
    assert_eq!(net.peer(0).document().text(), "localinitial!");
}

#[test]
fn reconnect_after_five_thousand_mutations() {
    let mut net = Network::new(2);
    for sequence in 1..=5_000 {
        let anchor = (sequence > 1).then(|| ElementId {
            replica: 1,
            sequence: sequence - 1,
            offset: 0,
        });
        net.peer_mut(0).insert(anchor, "x").unwrap();
    }
    net.peer_mut(1).insert(None, "offline").unwrap();
    net.repair(0, 1, 10);
    net.repair(1, 0, 10);
    net.drain().unwrap();
    assert!(net.converged());
    assert_eq!(
        net.peer(1).document().text(),
        format!("offline{}", "x".repeat(5_000))
    );
}
