#![allow(clippy::unwrap_used)]

//! Integration test: relay round-trip.
//!
//! Starts an in-process relay server, creates two relay transport clients,
//! sends a message from client A, and asserts client B receives it within
//! 5 seconds.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use ark_core::protocol::{deserialize_message, serialize_message, LanSyncMessage};

// ---------------------------------------------------------------------------
// Minimal in-process relay server
// ---------------------------------------------------------------------------

/// Per-space room: device_id → outbound sender.
type Room = HashMap<String, mpsc::UnboundedSender<Message>>;
type Rooms = Arc<Mutex<HashMap<String, Room>>>;

// Same tungstenite constraint as the in-crate test relay: accept_hdr_async's
// callback must return `Result<Response, ErrorResponse>` whose Err variant is
// the full HTTP error response.
#[allow(clippy::result_large_err)]
async fn start_relay(rooms: Rooms, listener: TcpListener) {
    loop {
        let (stream, _peer) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => break,
        };
        let rooms = rooms.clone();
        tokio::spawn(async move {
            // Parse space_id and device_id from the URL query string.
            let mut space_id = String::new();
            let mut device_id = String::new();

            let ws_stream = tokio_tungstenite::accept_hdr_async(
                stream,
                |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    let query = req.uri().query().unwrap_or("");
                    for pair in query.split('&') {
                        if let Some((k, v)) = pair.split_once('=') {
                            match k {
                                "space_id" => space_id = v.to_string(),
                                "device_id" => device_id = v.to_string(),
                                _ => {}
                            }
                        }
                    }
                    Ok(resp)
                },
            )
            .await
            .unwrap();

            let (mut ws_tx, mut ws_rx) = ws_stream.split();
            let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

            // Register in the room.
            {
                let mut rs = rooms.lock().unwrap();
                rs.entry(space_id.clone())
                    .or_default()
                    .insert(device_id.clone(), out_tx);
            }

            // Writer task.
            let mut write_task = tokio::spawn(async move {
                while let Some(msg) = out_rx.recv().await {
                    if ws_tx.send(msg).await.is_err() {
                        break;
                    }
                }
            });

            // Read + broadcast loop.
            loop {
                tokio::select! {
                    _ = &mut write_task => break,
                    frame = ws_rx.next() => {
                        match frame {
                            Some(Ok(msg @ Message::Text(_))) | Some(Ok(msg @ Message::Binary(_))) => {
                                let rs = rooms.lock().unwrap();
                                if let Some(room) = rs.get(&space_id) {
                                    for (dev, tx) in room {
                                        if dev != &device_id {
                                            let _ = tx.send(msg.clone());
                                        }
                                    }
                                }
                            }
                            Some(Ok(Message::Ping(d))) => {
                                // Ignored in test relay
                                let _ = d;
                            }
                            _ => break,
                        }
                    }
                }
            }

            // Unregister.
            let mut rs = rooms.lock().unwrap();
            if let Some(room) = rs.get_mut(&space_id) {
                room.remove(&device_id);
            }
            write_task.abort();
        });
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn relay_round_trip() {
    // 1. Start the in-process relay on a random port.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let relay_url = format!("ws://127.0.0.1:{port}");

    let rooms: Rooms = Arc::new(Mutex::new(HashMap::new()));
    let rooms_srv = rooms.clone();
    tokio::spawn(async move {
        start_relay(rooms_srv, listener).await;
    });

    // Allow the server a moment to initialize.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Create a channel for instance B to signal receipt.
    let (received_tx, mut received_rx) = mpsc::unbounded_channel::<LanSyncMessage>();

    // 3. Connect client B first.
    let space_id = "test-space-relay-round-trip";
    let url_b = format!("{relay_url}/ws?space_id={space_id}&device_id=device-B&api_key=");
    let (ws_b, _) = connect_async(&url_b).await.expect("B connect");
    let (_ws_b_tx, mut ws_b_rx) = ws_b.split();

    // Spawn B receiver.
    tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = ws_b_rx.next().await {
            if let Some(msg) = deserialize_message(&text) {
                let _ = received_tx.send(msg);
            }
        }
    });

    // 4. Connect client A.
    let url_a = format!("{relay_url}/ws?space_id={space_id}&device_id=device-A&api_key=");
    let (ws_a, _) = connect_async(&url_a).await.expect("A connect");
    let (mut ws_a_tx, _ws_a_rx) = ws_a.split();

    // Allow connections to settle.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 5. Instance A broadcasts a live_change entity.
    let test_entity = ark_core::types::SyncEntity {
        entity_type: "todo".to_string(),
        id: "test-entity-relay-001".to_string(),
        data: {
            let mut m = serde_json::Map::new();
            m.insert(
                "title".to_string(),
                serde_json::Value::String("Relay test task".to_string()),
            );
            m
        },
        hlc: "2026-04-09T00:00:00.000Z:000001:device-A".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };

    let change_id = "change-relay-test-001".to_string();
    let msg = LanSyncMessage::LiveChange {
        change_id,
        entity: test_entity.clone(),
        origin_device_id: Some("device-A".to_string()),
    };
    let text = serialize_message(&msg);
    ws_a_tx.send(Message::Text(text)).await.unwrap();

    // 6. Assert B receives the message within 5 seconds.
    let received = tokio::time::timeout(Duration::from_secs(5), received_rx.recv())
        .await
        .expect("timed out waiting for relay message")
        .expect("channel closed");

    match received {
        LanSyncMessage::LiveChange { entity, .. } => {
            assert_eq!(entity.id, "test-entity-relay-001", "entity id mismatch");
            assert_eq!(entity.entity_type, "todo", "entity type mismatch");
        }
        other => unreachable!("expected LiveChange, got {other:?}"),
    }
}
