//! Real UDP loopback experiment. Peers are Tokio tasks in one process.
#![recursion_limit = "256"]
use anyhow::{Result, anyhow};
use clap::Parser;
use conv_protocol::outbox::Outbox;
use conv_protocol::{
    sync::{Effect, Message, SyncConfig, SyncPeer},
    wire::{Reassembler, encode},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{
    net::UdpSocket,
    sync::{Barrier, watch},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    full_state: bool,
    #[arg(long)]
    network_bootstrap: bool,
    #[arg(long, default_value_t = 0)]
    delete_every: u64,
    #[arg(long, default_value_t = 1)]
    warmup_seconds: u64,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    #[arg(long, default_value_t = 10)]
    peers: usize,
    #[arg(long, default_value_t = 10)]
    writers: usize,
    #[arg(long, default_value_t = 5)]
    edits_per_second: u64,
    #[arg(long, default_value_t = 5)]
    seconds: u64,
    #[arg(long, default_value_t = 10)]
    settle_seconds: u64,
    #[arg(long, default_value_t = 1024)]
    document_bytes: usize,
    /// Deterministically discard this fraction of outbound frames (0..1,000,000).
    #[arg(long, default_value_t = 0)]
    drop_per_million: u64,
    /// Start a two-group network partition this many seconds into active editing.
    #[arg(long, default_value_t = 2)]
    partition_start_seconds: u64,
    #[arg(long, default_value_t = 0)]
    partition_seconds: u64,
    #[arg(long, default_value = "benchmarks/raw-results/latest.json")]
    output: std::path::PathBuf,
}

struct ResultRow {
    peer: SyncPeer,
    sent: u64,
    received: u64,
    sent_frames: u64,
    received_frames: u64,
    load_sent: u64,
    load_received: u64,
    local_edits: u64,
    attempted_frames: u64,
    loss_dropped_frames: u64,
    partition_dropped_frames: u64,
    merges: Vec<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.peers > 0
            && args.peers <= 500
            && args.edits_per_second > 0
            && args.edits_per_second <= 1000
    );
    anyhow::ensure!(
        !args.full_state || args.peers == 2,
        "the full-state baseline currently supports exactly two peers so forwarding topology remains identical"
    );
    anyhow::ensure!(args.drop_per_million <= 1_000_000);
    anyhow::ensure!(
        args.partition_seconds == 0
            || (args.peers >= 2
                && args.partition_start_seconds < args.seconds
                && args
                    .partition_start_seconds
                    .checked_add(args.partition_seconds)
                    .is_some_and(|end| end <= args.seconds)),
        "the partition must fit within the active editing window"
    );
    let writers = args.writers.min(args.peers);
    let room = uuid::Uuid::new_v4().as_u128();
    let mut sockets = Vec::new();
    for _ in 0..args.peers {
        sockets.push(UdpSocket::bind("127.0.0.1:0").await?);
    }
    let addresses: BTreeMap<_, _> = sockets
        .iter()
        .enumerate()
        .map(|(i, socket)| Ok((i as u128 + 1, socket.local_addr()?)))
        .collect::<Result<_>>()?;
    let mut bootstrap = conv_protocol::replication::Replica::new(u128::MAX);
    let initial = bootstrap
        .edit_utf16(0, 0, &"x".repeat(args.document_bytes))
        .map_err(|e| anyhow!("bootstrap: {e:?}"))?;
    let initial_mutations = initial.len();
    drop(bootstrap);
    let preparation = Instant::now();
    let ready = Arc::new(Barrier::new(args.peers + 1));
    let gate = Arc::new(Barrier::new(args.peers + 1));
    let clock = Arc::new(OnceLock::<Instant>::new());
    let edit_duration = Duration::from_secs(args.seconds);
    let (stop, receiver) = watch::channel(false);
    let mut tasks = Vec::new();
    for (index, socket) in sockets.into_iter().enumerate() {
        let (ready, gate, clock) = (ready.clone(), gate.clone(), clock.clone());
        let addresses = addresses.clone();
        let mut receiver = receiver.clone();
        let initial = initial.clone();
        let full_state = args.full_state;
        let delete_every = args.delete_every;
        let preload = !args.network_bootstrap || index == 0;
        let warmup = Duration::from_secs(args.warmup_seconds);
        let mut random = args.seed.wrapping_add(index as u64);
        let drop_per_million = args.drop_per_million;
        let partition_start = Duration::from_secs(args.partition_start_seconds);
        let partition_end = partition_start + Duration::from_secs(args.partition_seconds);
        let partition_enabled = args.partition_seconds > 0;
        let group_boundary = args.peers / 2;
        let edit_period = Duration::from_micros(1_000_000 / args.edits_per_second);
        tasks.push(tokio::spawn(async move {
            let id = index as u128 + 1;
            let mut neighbors: Vec<_> = addresses.keys().copied().filter(|&other| other != id).collect();
            neighbors.sort_by_cached_key(|_| { random = random.wrapping_mul(6364136223846793005).wrapping_add(1); random });
            let mut replica = conv_protocol::replication::Replica::new(id);
            if preload { for mutation in &initial { replica.receive(mutation).map_err(|e| anyhow!("bootstrap: {e:?}"))?; } }
            let mut peer = SyncPeer::from_replica(replica, neighbors, SyncConfig::default());
            drop(initial);
            let mut incoming = Reassembler::new(room);
            let mut buffer = [0_u8; 1_201];
            let mut load_sent = 0_u64; let mut load_received = 0_u64;
            let mut local_edits = 0_u64;
            let mut last_insert = None;
            let mut sequence = 0_u64; let mut sent = 0; let mut received = 0; let mut sent_frames = 0; let mut received_frames = 0; let mut merges = Vec::new();
            let mut attempted_frames = 0_u64; let mut loss_dropped_frames = 0_u64; let mut partition_dropped_frames = 0_u64;
            ready.wait().await; gate.wait().await;
            let start = *clock.get().unwrap();
            let mut sync = tokio::time::interval(Duration::from_secs(1));
            let mut probes = tokio::time::interval(Duration::from_millis(100));
            let mut batch_timer = tokio::time::interval(Duration::from_millis(20));
            let mut outbox = Outbox::default();
            let mut edits = tokio::time::interval_at(tokio::time::Instant::from_std(start) + warmup, edit_period);
            edits.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                let now = start.elapsed().as_millis() as u64;
                let mut flushed = false;
                let effects = tokio::select! {
                    _ = batch_timer.tick() => { flushed = true; outbox.flush() },
                    _ = receiver.changed() => break,
                    _ = sync.tick() => peer.on_sync_timer(),
                    _ = probes.tick() => {
                        if peer.needs_probe_order() {
                            let mut order: Vec<_> = addresses.keys().copied().filter(|&other| other != id).collect();
                            order.sort_by_cached_key(|_| { random = random.wrapping_mul(6364136223846793005).wrapping_add(1); random });
                            peer.reshuffle_probes(order);
                        }
                        peer.on_membership_timer(start.elapsed().as_millis() as u64)
                    },
                    _ = edits.tick(), if index < writers && start.elapsed() < edit_duration + warmup => {
                        if start.elapsed() < edit_duration + warmup {
                            local_edits += 1;
                            if delete_every > 0 && local_edits.is_multiple_of(delete_every) && last_insert.is_some() {
                                peer.delete(&[last_insert.take().unwrap()]).map_err(|e| anyhow!("delete: {e:?}"))?
                            } else {
                                let effects = peer.insert(None, "z").map_err(|e| anyhow!("edit: {e:?}"))?;
                                last_insert = Some(conv_document::ElementId { replica: id, sequence: peer.replica().local_sequence(), offset: 0 });
                                effects
                            }
                        } else { Vec::new() }
                    }
                    datagram = socket.recv_from(&mut buffer) => {
                        let (len, _) = datagram?;
                        if start.elapsed() >= warmup { received += len as u64; received_frames += 1;
                            if start.elapsed() < warmup + edit_duration { load_received += len as u64; }
                        }
                        if let Ok(Some((sender, message))) = incoming.receive(now, &buffer[..len]) {
                            let is_mutation = matches!(message, Message::Mutation(_) | Message::Batch(_));
                            let merge_start = Instant::now();
                            let effects = peer.receive_at(sender, message, now).map_err(|e| anyhow!("receive: {e:?}"))?;
                            if is_mutation && start.elapsed() >= warmup { merges.push(merge_start.elapsed().as_nanos() as u64); }
                            effects
                        } else { Vec::new() }
                    }
                };
                let effects = if flushed { effects } else { outbox.push(effects) };
                for effect in effects {
                    if let Effect::Send { to, mut message } = effect {
                        if full_state && matches!(message, Message::Mutation(_) | Message::Batch(_)) {
                            message = Message::Snapshot(peer.replica().snapshot());
                        }
                        sequence += 1;
                        let frames = encode(room, id, sequence, &message).map_err(|e| anyhow!("encode: {e:?}"))?;
                        for frame in frames {
                            let elapsed = start.elapsed();
                            let measured = elapsed >= warmup;
                            if measured { attempted_frames += 1; }
                            let active = elapsed.saturating_sub(warmup);
                            if partition_enabled && measured && active >= partition_start && active < partition_end
                                && (index < group_boundary) != ((to as usize - 1) < group_boundary) {
                                if measured { partition_dropped_frames += 1; }
                                continue;
                            }
                            if drop_per_million > 0 {
                                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                                if random % 1_000_000 < drop_per_million {
                                    if measured { loss_dropped_frames += 1; }
                                    continue;
                                }
                            }
                            let bytes = socket.send_to(&frame, addresses[&to]).await? as u64;
                            if start.elapsed() >= warmup { sent += bytes; sent_frames += 1;
                                if start.elapsed() < warmup + edit_duration { load_sent += bytes; }
                            }
                        }
                    }
                }
            }
            Ok::<_, anyhow::Error>(ResultRow { peer, sent, received, sent_frames, received_frames, load_sent, load_received, local_edits, attempted_frames, loss_dropped_frames, partition_dropped_frames, merges })
        }));
    }
    ready.wait().await;
    let initialization_seconds = preparation.elapsed().as_secs_f64();
    let start = Instant::now();
    clock.set(start).unwrap();
    gate.wait().await;
    tokio::time::sleep(
        edit_duration + Duration::from_secs(args.settle_seconds + args.warmup_seconds),
    )
    .await;
    stop.send(true)?;
    let mut rows = Vec::new();
    for task in tasks {
        rows.push(task.await??);
    }
    let elapsed = start.elapsed().as_secs_f64() - args.warmup_seconds as f64;
    let converged = rows.windows(2).all(|pair| {
        pair[0].peer.replica().document() == pair[1].peer.replica().document()
            && pair[0].peer.replica().summary() == pair[1].peer.replica().summary()
    });
    let generated_mutations: u64 = rows.iter().map(|row| row.local_edits).sum();
    let requested_mutations = writers as u64 * args.edits_per_second * args.seconds;
    let expected_mutations = initial_mutations as u64 + generated_mutations;
    let complete_history = rows
        .iter()
        .all(|row| row.peer.replica().history_len() as u64 == expected_mutations);
    let all_writers_participated =
        args.seconds == 0 || rows[..writers].iter().all(|row| row.local_edits > 0);
    let mut merges: Vec<_> = rows
        .iter_mut()
        .flat_map(|row| std::mem::take(&mut row.merges))
        .collect();
    merges.sort_unstable();
    let percentile = |p: usize| {
        merges
            .get((merges.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
            .unwrap_or(0)
    };
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let toolchain = std::process::Command::new("rustc")
        .arg("--version")
        .output()?;
    let report = serde_json::json!({
        "commit":String::from_utf8_lossy(&commit.stdout).trim(), "toolchain":String::from_utf8_lossy(&toolchain.stdout).trim(),
        "mode":if args.full_state {"full-state"} else {"delta"}, "seed":args.seed,"warmup_seconds":args.warmup_seconds,"network_bootstrap":args.network_bootstrap,"bootstrap_mutations":initial_mutations,
        "hardware":std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default().lines().find(|line| line.starts_with("model name")).unwrap_or("unknown"),
        "process_memory":std::fs::read_to_string("/proc/self/status").unwrap_or_default().lines().filter(|line| line.starts_with("VmHWM") || line.starts_with("VmRSS")).collect::<Vec<_>>(),
        "source_dirty":!std::process::Command::new("git").args(["status", "--porcelain", "--untracked-files=no"]).output()?.stdout.is_empty(),
        "transport_overhead":"28 bytes per IPv4/UDP datagram, reported separately from application bytes",
        "transport":"UDP loopback", "execution":"Tokio tasks in one process", "peers":args.peers,
        "writers":writers,"delete_every":args.delete_every,"edits_per_second_per_writer":args.edits_per_second,"merge_sample_unit":"decoded mutation or batch message","edit_seconds":args.seconds,"settle_seconds":args.settle_seconds,
        "requested_live_mutations":requested_mutations,"generated_live_mutations":generated_mutations,"expected_mutations_per_replica":expected_mutations,
        "achieved_mutations_per_second":generated_mutations as f64 / args.seconds.max(1) as f64,
        "all_writers_participated":all_writers_participated,"complete_history":complete_history,"verified":converged && complete_history && all_writers_participated,
        "faults":{"drop_per_million":args.drop_per_million,"partition_start_seconds":args.partition_start_seconds,"partition_seconds":args.partition_seconds,"partition_groups":"first floor(peers/2) ids versus remaining ids; outbound cross-group frames blocked","injection":"seeded application-level frame discard before real UDP send; no emulated RTT","counts_scope":"active edit and settling windows, excluding warmup"},
        "document_bytes":args.document_bytes,"initialization_seconds":initialization_seconds,"load_executed":all_writers_participated,"elapsed_seconds":elapsed,"converged":converged,
        "bootstrap":if args.network_bootstrap {"initial history only on peer 1; bootstrap bytes included after the configured warmup"} else {"identical initial state preloaded before network measurement"}, "measurement":"sent and received application frame bytes including headers, control and repair; excludes IP/UDP headers",
        "per_node": rows.iter().map(|row| serde_json::json!({"sent":row.sent,"received":row.received,"sent_frames":row.sent_frames,"received_frames":row.received_frames,"local_mutations":row.local_edits,"attempted_frames":row.attempted_frames,"loss_dropped_frames":row.loss_dropped_frames,"partition_dropped_frames":row.partition_dropped_frames,"ipv4_udp_overhead":28 * (row.sent_frames + row.received_frames),"visible_bytes":row.peer.replica().document().text().len(),"tombstones":row.peer.replica().document().deleted_ids().count(),"serialized_history_bytes":row.peer.replica().backup(room).map(|bytes| bytes.len()).unwrap_or(0),"bytes_per_second":(row.sent+row.received) as f64/elapsed,"active_window_bytes_per_second":(row.load_sent+row.load_received) as f64 / args.seconds.max(1) as f64,"active_window_sent":row.load_sent,"active_window_received":row.load_received,"mutations":row.peer.replica().history_len()})).collect::<Vec<_>>(),
        "protocol_mutation_merge_ns":{"samples":merges.len(),"p50":percentile(50),"p95":percentile(95),"p99":percentile(99)},
        "limitations":["single-host loopback", "merge includes protocol history bookkeeping", "CPU time requires the runner resource sidecar", "not a browser or WAN result"]
    });
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{} peers, converged={converged}; report {}",
        args.peers,
        args.output.display()
    );
    anyhow::ensure!(
        all_writers_participated,
        "not every configured writer executed its workload"
    );
    anyhow::ensure!(
        converged,
        "replicas did not converge before the experiment deadline"
    );
    anyhow::ensure!(
        complete_history,
        "replicas do not retain every generated mutation"
    );
    Ok(())
}
