//! ark-relay-server: WebSocket relay for Kepler P2P mesh.
//!
//! Forwards `LanSyncMessage` JSON frames between devices sharing the same
//! `space_id`. Persists an event log to SQLite so new devices receive a
//! catch-up replay on connect.
//!
//! Usage:
//!   ARK_RELAY_PORT=8765 ARK_RELAY_DB=relay.db ARK_RELAY_API_KEY=secret \
//!     ark-relay-server
//!
//! WebSocket endpoint: GET /ws?space_id=X&device_id=Y&api_key=Z

mod db;
mod relay;

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

use crate::db::Database;
use crate::relay::RelayRooms;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("ARK_RELAY_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8765);

    let db_path = std::env::var("ARK_RELAY_DB").unwrap_or_else(|_| "relay.db".to_string());

    let api_key = std::env::var("ARK_RELAY_API_KEY").ok();

    let db = Database::open(&db_path).expect("open relay db");
    let rooms = RelayRooms::new(db);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("bind");
    eprintln!("[ark-relay-server] listening on {addr}");

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ark-relay-server] accept error: {e}");
                continue;
            }
        };

        let rooms = rooms.clone();
        let api_key = api_key.clone();

        tokio::spawn(async move {
            // Parse query string from the HTTP upgrade request.
            let mut space_id = String::new();
            let mut device_id = String::new();
            let mut req_api_key = String::new();

            let ws_stream = accept_hdr_async(stream, |req: &Request, response: Response| {
                let uri = req.uri();
                let query = uri.query().unwrap_or("");
                for pair in query.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        match k {
                            "space_id" => space_id = v.to_string(),
                            "device_id" => device_id = v.to_string(),
                            "api_key" => req_api_key = v.to_string(),
                            _ => {}
                        }
                    }
                }
                Ok(response)
            })
            .await;

            let ws_stream = match ws_stream {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("[relay] ws handshake failed from {peer_addr}: {e}");
                    return;
                }
            };

            // Validate api_key if configured.
            if let Some(ref expected) = api_key {
                if req_api_key != *expected {
                    eprintln!("[relay] rejected device={device_id}: bad api_key");
                    return;
                }
            }

            if space_id.is_empty() || device_id.is_empty() {
                eprintln!("[relay] rejected from {peer_addr}: missing space_id or device_id");
                return;
            }

            eprintln!("[relay] connected device={device_id} space={space_id}");
            relay::handle_connection(ws_stream, space_id, device_id, rooms).await;
        });
    }
}
