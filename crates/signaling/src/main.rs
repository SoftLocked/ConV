//! Ephemeral metadata only. Document synchronization is carried by WebRTC peers.
use axum::{
    Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Instant};
use tokio::sync::{Mutex, mpsc};

#[derive(Clone)]
struct App {
    rooms: Arc<Mutex<BTreeMap<String, BTreeMap<String, Client>>>>,
    origin: String,
}
#[derive(Clone)]
struct Client {
    name: String,
    send: mpsc::Sender<String>,
}
#[derive(Deserialize)]
struct Join {
    room: String,
    session: String,
    name: String,
}
#[derive(Serialize)]
struct Peer {
    id: String,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Signal {
    to: String,
    description: Option<Description>,
    candidate: Option<Candidate>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    #[serde(rename = "type")]
    kind: String,
    sdp: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Candidate {
    candidate: String,
    sdp_mid: Option<String>,
    sdp_m_line_index: Option<u16>,
    username_fragment: Option<String>,
}

fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

async fn join(
    State(app): State<App>,
    Query(join): Query<Join>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    if headers.get("origin").and_then(|h| h.to_str().ok()) != Some(app.origin.as_str()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !valid_id(&join.room)
        || !valid_id(&join.session)
        || join.name.chars().count() > 40
        || join.name.is_empty()
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    ws.max_message_size(32_768)
        .max_frame_size(32_768)
        .on_upgrade(move |socket| session(socket, app, join))
        .into_response()
}

async fn session(socket: WebSocket, app: App, join: Join) {
    let (send, mut receive) = mpsc::channel::<String>(64);
    let (mut outgoing, mut incoming) = socket.split();
    {
        let mut rooms = app.rooms.lock().await;
        if !rooms.contains_key(&join.room) && rooms.len() >= 1_000 {
            return;
        }
        let room = rooms.entry(join.room.clone()).or_default();
        if room.len() >= 8 || room.contains_key(&join.session) {
            return;
        }
        let peers: Vec<_> = room
            .iter()
            .map(|(id, client)| Peer {
                id: id.clone(),
                name: client.name.clone(),
            })
            .collect();
        let welcome = serde_json::json!({"type":"welcome", "peers":peers}).to_string();
        let _ = send.try_send(welcome);
        let joined = serde_json::json!({"type":"join", "peer":Peer { id: join.session.clone(), name: join.name.clone() }}).to_string();
        for peer in room.values() {
            let _ = peer.send.try_send(joined.clone());
        }
        room.insert(
            join.session.clone(),
            Client {
                name: join.name.clone(),
                send,
            },
        );
    }
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(15));
    let mut last_seen = Instant::now();
    let mut window = Instant::now();
    let mut count = 0_u32;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if last_seen.elapsed().as_secs() > 45 { break; }
                if outgoing.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
            pending = receive.recv() => match pending {
                Some(text) => if outgoing.send(Message::Text(text.into())).await.is_err() { break; },
                None => break,
            },
            event = incoming.next() => match event {
                Some(Ok(Message::Text(text))) => {
                    last_seen = Instant::now();
                    if window.elapsed().as_secs() >= 1 { count = 0; window = Instant::now(); }
                    count += 1;
                    if count > 60 { break; }
                    let Ok(signal) = serde_json::from_str::<Signal>(&text) else { break; };
                    if !valid_id(&signal.to) || signal.description.is_some() == signal.candidate.is_some() { break; }
                    if let Some(description) = &signal.description
                        && (!matches!(description.kind.as_str(), "offer" | "answer") || description.sdp.len() > 24_000) { break; }
                    if let Some(candidate) = &signal.candidate
                        && candidate.candidate.len() > 4_096 { break; }
                    let relay = serde_json::json!({"type":"signal", "from":join.session, "description":signal.description, "candidate":signal.candidate}).to_string();
                    let rooms = app.rooms.lock().await;
                    if let Some(target) = rooms.get(&join.room).and_then(|r| r.get(&signal.to)) { let _ = target.send.try_send(relay); }
                }
                Some(Ok(Message::Pong(_))) | Some(Ok(Message::Ping(_))) => { last_seen = Instant::now(); }
                Some(Ok(Message::Binary(_))) | Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
            }
        }
    }
    let mut rooms = app.rooms.lock().await;
    if let Some(room) = rooms.get_mut(&join.room) {
        room.remove(&join.session);
        let left = serde_json::json!({"type":"leave", "id":join.session}).to_string();
        for peer in room.values() {
            let _ = peer.send.try_send(left.clone());
        }
        if room.is_empty() {
            rooms.remove(&join.room);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let bind = std::env::var("CONV_SIGNAL_BIND").unwrap_or_else(|_| "127.0.0.1:3001".into());
    let app = App {
        rooms: Arc::default(),
        origin: std::env::var("CONV_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into()),
    };
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/signal", get(join))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("ConV signaling listening on {bind}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
