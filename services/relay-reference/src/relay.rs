//! Room/space management for the relay server.
//!
//! Each "room" corresponds to one `space_id`. Connections are identified by
//! `device_id`. Messages are fanned out to all other devices in the same room.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::db::Database;

// ---------------------------------------------------------------------------
// Per-connection sender alias
// ---------------------------------------------------------------------------

type WsSender = mpsc::UnboundedSender<Message>;

// ---------------------------------------------------------------------------
// Room state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RelayRooms {
    /// space_id → { device_id → WsSender }
    inner: Arc<Mutex<HashMap<String, HashMap<String, WsSender>>>>,
    /// Shared SQLite database wrapped in Arc so multiple tasks can read.
    db: Arc<Mutex<Database>>,
}

impl RelayRooms {
    pub fn new(db: Database) -> Self {
        RelayRooms {
            inner: Arc::new(Mutex::new(HashMap::new())),
            db: Arc::new(Mutex::new(db)),
        }
    }

    fn join(&self, space_id: &str, device_id: &str, tx: WsSender) {
        let mut rooms = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        rooms
            .entry(space_id.to_string())
            .or_default()
            .insert(device_id.to_string(), tx);
    }

    fn leave(&self, space_id: &str, device_id: &str) {
        let mut rooms = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(room) = rooms.get_mut(space_id) {
            room.remove(device_id);
            if room.is_empty() {
                rooms.remove(space_id);
            }
        }
    }

    /// Broadcast a text frame to all devices in the room **except** `from_device`.
    fn broadcast(&self, space_id: &str, from_device: &str, msg: Message) {
        let rooms = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(room) = rooms.get(space_id) {
            for (dev, tx) in room.iter() {
                if dev != from_device {
                    let _ = tx.send(msg.clone());
                }
            }
        }
    }

    /// Return catch-up messages for a new device.
    fn catch_up(&self, space_id: &str) -> Vec<Message> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        match db.get_events_since(space_id, "") {
            Ok(rows) => rows
                .into_iter()
                .map(|r| {
                    // Re-wrap entity_json as a live_change message.
                    let payload = serde_json::json!({
                        "type": "live_change",
                        "change_id": format!("catchup-{}", r.id),
                        "entity": serde_json::from_str::<Value>(&r.entity_json)
                            .unwrap_or(Value::Null),
                    });
                    Message::Text(payload.to_string())
                })
                .collect(),
            Err(e) => {
                eprintln!("[relay] db catch-up error: {e}");
                vec![]
            }
        }
    }

    /// Store an entity from a `live_change` frame to the event log.
    fn store_entity(&self, space_id: &str, device_id: &str, frame: &str) {
        // Parse the outer wrapper to extract entity and hlc.
        let Ok(v) = serde_json::from_str::<Value>(frame) else {
            return;
        };
        if v.get("type").and_then(Value::as_str) != Some("live_change") {
            return;
        }
        let entity = &v["entity"];
        let hlc = entity
            .get("hlc")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let entity_json = serde_json::to_string(entity).unwrap_or_default();
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = db.store_event(space_id, device_id, &entity_json, &hlc) {
            eprintln!("[relay] db store error: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket connection handler
// ---------------------------------------------------------------------------

/// Handle a single WebSocket connection.
///
/// Query params expected: `space_id`, `device_id`, `api_key`.
pub async fn handle_connection(
    ws_stream: WebSocketStream<TcpStream>,
    space_id: String,
    device_id: String,
    rooms: RelayRooms,
) {
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Per-connection outbound channel.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
    rooms.join(&space_id, &device_id, out_tx.clone());

    // Send catch-up messages.
    for msg in rooms.catch_up(&space_id) {
        let _ = out_tx.send(msg);
    }

    // Spawn writer task.
    let mut write_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read loop — fan out incoming frames.
    loop {
        tokio::select! {
            _ = &mut write_task => break,
            frame = ws_rx.next() => {
                match frame {
                    Some(Ok(Message::Text(text))) => {
                        let text_str = text.to_string();
                        // Persist entity changes.
                        rooms.store_entity(&space_id, &device_id, &text_str);
                        // Broadcast to peers.
                        rooms.broadcast(&space_id, &device_id, Message::Text(text));
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        rooms.broadcast(&space_id, &device_id, Message::Binary(bin));
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = out_tx.send(Message::Pong(data));
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        eprintln!("[relay] ws error device={device_id}: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    rooms.leave(&space_id, &device_id);
    write_task.abort();
}
