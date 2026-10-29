use conv_protocol::membership::{Member, Status, Swim, SwimMessage};

#[test]
fn failed_direct_and_indirect_probes_suspect_then_expire() {
    let mut swim = Swim::new(1);
    swim.set_probe_order(vec![2, 3]);
    assert!(matches!(swim.tick(0)[0].message, SwimMessage::Ping { .. }));
    assert!(
        swim.tick(250)
            .iter()
            .any(|send| matches!(send.message, SwimMessage::PingReq { target: 2, .. }))
    );
    swim.tick(500);
    assert_eq!(swim.members()[&2].status, Status::Suspect);
    swim.tick(5_500);
    assert_eq!(swim.members()[&2].status, Status::Dead);
    swim.observe(
        Member {
            id: 2,
            incarnation: 0,
            status: Status::Alive,
        },
        6_000,
    );
    assert_eq!(swim.members()[&2].status, Status::Dead);
    swim.observe(
        Member {
            id: 2,
            incarnation: 1,
            status: Status::Alive,
        },
        6_000,
    );
    assert_eq!(swim.members()[&2].status, Status::Alive);
}

#[test]
fn suspicion_refutation_and_stale_timer_do_not_kill_live_peer() {
    let mut swim = Swim::new(1);
    swim.set_probe_order(vec![2]);
    let refutation = swim.observe(
        Member {
            id: 1,
            incarnation: 2,
            status: Status::Suspect,
        },
        0,
    );
    assert_eq!(swim.members()[&1].incarnation, 3);
    assert!(matches!(
        refutation[0].message,
        SwimMessage::Update(Member {
            incarnation: 3,
            status: Status::Alive,
            ..
        })
    ));
    swim.observe(
        Member {
            id: 2,
            incarnation: 0,
            status: Status::Suspect,
        },
        0,
    );
    swim.observe(
        Member {
            id: 2,
            incarnation: 1,
            status: Status::Alive,
        },
        1,
    );
    swim.tick(5_000);
    assert_eq!(swim.members()[&2].status, Status::Alive);
}

#[test]
fn indirect_ack_reaches_original_requester() {
    let mut a = Swim::new(1);
    a.set_probe_order(vec![2, 3]);
    let mut helper = Swim::new(3);
    helper.set_probe_order(vec![1, 2]);
    let mut target = Swim::new(2);
    a.tick(0);
    let request = a.tick(250).remove(0);
    let ping = helper.receive(1, request.message, 250).remove(0);
    let ack = target.receive(3, ping.message, 260).remove(0);
    let forwarded = helper.receive(2, ack.message, 270).remove(0);
    a.receive(3, forwarded.message, 280);
    a.tick(500);
    assert_eq!(a.members()[&2].status, Status::Alive);
}
