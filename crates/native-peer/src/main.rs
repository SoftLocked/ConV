use anyhow::{Context, Result, anyhow};
use clap::Parser;
use conv_protocol::{
    replication::Replica,
    sync::{Effect, Message, SyncConfig, SyncPeer},
    wire::{Reassembler, encode},
};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::UdpSocket,
};

#[derive(Parser)]
#[command(about = "ConV UDP peer. Type a line to append; /show prints the document.")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:7001")]
    bind: SocketAddr,
    #[arg(long)]
    seed: Vec<SocketAddr>,
    #[arg(long, default_value = "1")]
    room: u128,
    #[arg(long, default_value = "peer.conv")]
    state: PathBuf,
}

fn save(peer: &SyncPeer, room: u128, path: &Path) -> Result<()> {
    let bytes = peer
        .replica()
        .backup(room)
        .map_err(|e| anyhow!("backup: {e:?}"))?;
    let temporary = path.with_extension("conv.tmp");
    let file = std::fs::File::create(&temporary)?;
    use std::io::Write;
    (&file).write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)?;
    std::fs::File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?
    .sync_all()?;
    Ok(())
}

struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn send(
    socket: &UdpSocket,
    room: u128,
    writer: u128,
    sequence: &mut u64,
    to: SocketAddr,
    message: &Message,
) -> Result<()> {
    *sequence = sequence
        .checked_add(1)
        .context("message counter exhausted")?;
    for frame in encode(room, writer, *sequence, message).map_err(|e| anyhow!("encode: {e:?}"))? {
        socket.send_to(&frame, to).await?;
    }
    Ok(())
}

async fn dispatch(
    socket: &UdpSocket,
    args: &Args,
    writer: u128,
    sequence: &mut u64,
    addresses: &BTreeMap<u128, SocketAddr>,
    effects: Vec<Effect>,
) -> Result<()> {
    for effect in effects {
        if let Effect::Send { to, message } = effect
            && let Some(address) = addresses.get(&to)
        {
            send(socket, args.room, writer, sequence, *address, &message).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let lock_path = args.state.with_extension("conv.lock");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .context("state is in use, or a stale .conv.lock remains after a crash")?;
    let _lock = Lock(lock_path);
    let writer = uuid::Uuid::new_v4().as_u128();
    let replica = match std::fs::read(&args.state) {
        Ok(bytes) => {
            Replica::restore(writer, args.room, &bytes).map_err(|e| anyhow!("restore: {e:?}"))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Replica::new(writer),
        Err(e) => return Err(e.into()),
    };
    let mut peer = SyncPeer::from_replica(replica, Vec::new(), SyncConfig::default());
    let socket = UdpSocket::bind(args.bind).await?;
    let mut addresses = BTreeMap::new();
    let mut incoming = Reassembler::new(args.room);
    let mut sequence = 0;
    let mut frame = [0_u8; 1_201];
    let start = Instant::now();
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut input_open = true;
    println!(
        "ConV listening on {} (room {}). /show, /quit, or type text.",
        socket.local_addr()?,
        args.room
    );
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = timer.tick() => {
                for &seed in &args.seed { send(&socket, args.room, writer, &mut sequence, seed, &Message::Hello).await?; }
                dispatch(&socket, &args, writer, &mut sequence, &addresses, peer.on_sync_timer()).await?;
                incoming.expire(start.elapsed().as_millis() as u64);
            }
            received = socket.recv_from(&mut frame) => {
                let (len, address) = received?;
                if let Ok(Some((sender, message))) = incoming.receive(start.elapsed().as_millis() as u64, &frame[..len]) {
                    if sender == writer { continue; }
                    if addresses.len() >= 256 && !addresses.contains_key(&sender) { continue; }
                    addresses.insert(sender, address);
                    peer.set_neighbors(addresses.keys().copied().collect());
                    let before = peer.replica().history_len();
                    match peer.receive(sender, message) {
                        Ok(effects) => {
                            if peer.replica().history_len() != before {
                                save(&peer, args.room, &args.state)?;
                                println!("{}", peer.replica().document().text());
                            }
                            dispatch(&socket, &args, writer, &mut sequence, &addresses, effects).await?;
                        }
                        Err(error) => eprintln!("Rejected message: {error:?}"),
                    }
                }
            }
            line = lines.next_line(), if input_open => {
                match line? {
                    Some(line) if line == "/quit" => break,
                    Some(line) if line == "/show" => println!("{}", peer.replica().document().text()),
                    Some(line) => {
                        let anchor = peer.replica().document().visible_elements().last().map(|&(id, _)| id);
                        let effects = peer.insert(anchor, &(line + "\n")).map_err(|e| anyhow!("edit: {e:?}"))?;
                        save(&peer, args.room, &args.state)?;
                        dispatch(&socket, &args, writer, &mut sequence, &addresses, effects).await?;
                        println!("Saved on this device");
                    }
                    None => input_open = false,
                }
            }
        }
    }
    save(&peer, args.room, &args.state)?;
    Ok(())
}
