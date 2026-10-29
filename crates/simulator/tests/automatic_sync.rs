use conv_document::ElementId;
use conv_protocol::sync::SyncConfig;
use conv_simulator::sync_network::{Faults, SyncNetwork};

#[test]
fn three_partitioned_writers_recover_without_manual_repair() {
    let mut net = SyncNetwork::new(3, 42, SyncConfig::default(), Faults::default());
    net.insert(0, None, "ab").unwrap();
    net.run_until(100).unwrap();
    assert!(net.converged());
    for a in 0..3 {
        for b in a + 1..3 {
            net.partition(a, b);
        }
    }
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
    net.insert(0, Some(a), "X").unwrap();
    net.insert(1, Some(a), "🦀").unwrap();
    net.delete(2, &[b]).unwrap();
    net.run_until(2_000).unwrap();
    assert!(!net.converged());
    net.heal();
    net.run_until(5_000).unwrap();
    assert!(net.converged());
    assert_eq!(net.peer(0).replica().document().text(), "a🦀X");
    assert!(net.requests_delivered > 0);
}

#[test]
fn seeded_loss_reordering_and_duplicates_eventually_repair() {
    let mut repair_requests = 0;
    for seed in 0..100 {
        let mut net = SyncNetwork::new(
            3,
            seed,
            SyncConfig {
                interval_ms: 100,
                max_ranges: 2,
                max_reply_items: 3,
            },
            Faults {
                max_delay_ms: 200,
                loss_per_thousand: 250,
                duplicate_per_thousand: 400,
            },
        );
        for peer in 0..3 {
            for _ in 0..10 {
                net.insert(peer, None, "x").unwrap();
            }
        }
        net.run_until(10_000).unwrap();
        assert!(net.converged(), "seed {seed}");
        assert_eq!(net.peer(0).replica().history_len(), 30, "seed {seed}");
        assert!(net.dropped > 0);
        repair_requests += net.requests_delivered;
    }
    assert!(repair_requests > 0);
}

#[test]
fn a_large_gap_is_repaired_in_bounded_rounds() {
    let mut net = SyncNetwork::new(
        2,
        7,
        SyncConfig {
            interval_ms: 100,
            max_ranges: 1,
            max_reply_items: 4,
        },
        Faults::default(),
    );
    net.partition(0, 1);
    for _ in 0..100 {
        net.insert(0, None, "x").unwrap();
    }
    net.insert(1, None, "offline").unwrap();
    net.run_until(500).unwrap();
    net.heal();
    net.run_until(1_000).unwrap();
    assert!(!net.converged()); // Bounded replies require more rounds.
    net.run_until(6_000).unwrap();
    assert!(net.converged());
    assert_eq!(net.peer(1).replica().history_len(), 101);
}
