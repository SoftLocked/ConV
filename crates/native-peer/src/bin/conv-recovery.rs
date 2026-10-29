//! Real UDP reconnect trials. Percentiles require 1,000 successful measured trials.
use anyhow::{Result, anyhow};
use clap::Parser;
use conv_protocol::outbox::Outbox;
use conv_protocol::{
    replication::Replica,
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
    sync::{Barrier, mpsc, watch},
};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 1000)]
    trials: usize,
    #[arg(long, default_value_t = 10)]
    warmup: usize,
    #[arg(long, default_value_t = 5000)]
    mutations: u64,
    #[arg(long, default_value_t = 8)]
    peers: usize,
    #[arg(long, default_value = "benchmarks/raw-results/recovery.json")]
    output: std::path::PathBuf,
}

async fn trial(source: &Replica, count: usize) -> Result<Option<u64>> {
    let room = uuid::Uuid::new_v4().as_u128();
    let mut sockets = Vec::new();
    for _ in 0..count {
        sockets.push(UdpSocket::bind("127.0.0.1:0").await?);
    }
    let addresses: BTreeMap<_, _> = sockets
        .iter()
        .enumerate()
        .map(|(i, socket)| Ok((i as u128 + 1, socket.local_addr()?)))
        .collect::<Result<_>>()?;
    let expected = Arc::new(source.clone());
    let ready = Arc::new(Barrier::new(count + 1));
    let gate = Arc::new(Barrier::new(count + 1));
    let clock = Arc::new(OnceLock::<Instant>::new());
    let (complete, mut completed) = mpsc::channel(count);
    let (stop, receiver) = watch::channel(false);
    let mut tasks = Vec::new();
    for (index, socket) in sockets.into_iter().enumerate() {
        let (expected, addresses, ready, gate, clock, complete, mut receiver) = (
            expected.clone(),
            addresses.clone(),
            ready.clone(),
            gate.clone(),
            clock.clone(),
            complete.clone(),
            receiver.clone(),
        );
        tasks.push(tokio::spawn(async move {
            let id = index as u128 + 1;
            let replica = if index == 0 { (*expected).clone() } else { Replica::new(id) };
            let mut peer = SyncPeer::from_replica(replica, addresses.keys().copied().filter(|&other| other != id).collect(), SyncConfig::default());
            let mut incoming = Reassembler::new(room);
            let mut bytes = [0_u8; 1_201];
            let mut number = 0; let mut reported = index == 0;
            let mut timer = tokio::time::interval(Duration::from_secs(1));
            let mut probes = tokio::time::interval(Duration::from_millis(100));
            let mut batch_timer = tokio::time::interval(Duration::from_millis(20));
            let mut outbox = Outbox::default();
            ready.wait().await; gate.wait().await;
            loop {
                let now = clock.get().unwrap().elapsed().as_millis() as u64;
                let mut flushed = false;
                let effects = tokio::select! {
                    _ = batch_timer.tick() => { flushed = true; outbox.flush() },
                    _ = receiver.changed() => break,
                    _ = probes.tick() => peer.on_membership_timer(now),
                    _ = timer.tick() => {
                        let mut effects = peer.on_sync_timer();
                        if index != 0 { effects.push(Effect::Send { to: 1, message: Message::Hello }); }
                        effects
                    }
                    datagram = socket.recv_from(&mut bytes) => {
                        let (len, _) = datagram?;
                        if let Ok(Some((sender, message))) = incoming.receive(now, &bytes[..len]) {
                            peer.receive_at(sender, message, now).map_err(|e| anyhow!("receive: {e:?}"))?
                        } else { Vec::new() }
                    }
                };
                if !reported && peer.replica().summary() == expected.summary() && peer.replica().document() == expected.document() {
                    reported = true; complete.send(index).await?;
                }
                let effects = if flushed { effects } else { outbox.push(effects) };
                for effect in effects {
                    if let Effect::Send { to, message } = effect {
                        number += 1;
                        for frame in encode(room, id, number, &message).map_err(|e| anyhow!("encode: {e:?}"))? {
                            socket.send_to(&frame, addresses[&to]).await?;
                        }
                    }
                }
            }
            Ok::<_, anyhow::Error>(())
        }));
    }
    ready.wait().await;
    let start = Instant::now();
    clock.set(start).unwrap();
    gate.wait().await;
    let success = tokio::time::timeout(Duration::from_secs(10), async {
        for _ in 1..count {
            completed
                .recv()
                .await
                .ok_or_else(|| anyhow!("peer stopped"))?;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await;
    let elapsed = start.elapsed().as_micros() as u64;
    stop.send(true)?;
    for task in tasks {
        task.await??;
    }
    Ok(success.ok().transpose()?.map(|_| elapsed))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.peers >= 2 && args.peers <= 8 && args.mutations > 0 && args.mutations <= 5000
    );
    let mut source = Replica::new(1);
    let mut anchor = None;
    for _ in 0..args.mutations {
        let mutation = source
            .insert(anchor, "x")
            .map_err(|e| anyhow!("edit: {e:?}"))?
            .unwrap();
        anchor = mutation
            .fragment
            .insertions()
            .last()
            .map(|record| record.id);
    }
    let mut samples = Vec::new();
    let mut failed = 0;
    for index in 0..args.trials + args.warmup {
        let elapsed = trial(&source, args.peers).await?;
        if index >= args.warmup {
            if let Some(elapsed) = elapsed {
                samples.push(elapsed);
            } else {
                failed += 1;
            }
        }
        if index % 100 == 0 {
            eprintln!("recovery trial {index}/{}", args.trials + args.warmup);
        }
    }
    samples.sort_unstable();
    let qualified = samples.len() >= 1000 && failed == 0;
    let p99 = qualified.then(|| samples[(samples.len() - 1) * 99 / 100]);
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    let report = serde_json::json!({"commit":String::from_utf8_lossy(&commit.stdout).trim(),"peers":args.peers,"offline_mutations":args.mutations,
        "toolchain":String::from_utf8_lossy(&std::process::Command::new("rustc").arg("--version").output()?.stdout).trim(),
        "hardware":std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default().lines().find(|line| line.starts_with("model name")).unwrap_or("unknown"),
        "source_dirty":!std::process::Command::new("git").args(["status", "--porcelain", "--untracked-files=no"]).output()?.stdout.is_empty(),
        "seed":0,"transport":"UDP loopback", "execution":"Tokio tasks in one process", "warmup_trials":args.warmup,
        "successful_trials":samples.len(),"failed_trials":failed,"recovery_microseconds":samples,"qualified_p99_microseconds":p99,
        "contract":"writers stopped; start before releasing restored reachability barrier; finish when every target has identical document state and causal summary",
        "limits":["single host", "eight peers maximum", "native only", "not a WAN or browser measurement"]});
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{} successful trials, {failed} failures; report {}",
        samples.len(),
        args.output.display()
    );
    anyhow::ensure!(failed == 0, "recovery failures recorded");
    Ok(())
}
