//! End-to-end sync integration test (AC20 / AC18 / AC19).
//!
//! Spins up two `SyncServer` instances in-process on loopback, dials one from
//! the other via `SyncClient`, and walks through a full protocol exchange:
//! `hello` → `version_vector` → `sync_changes` + ACK → `live_change` +
//! `live_ack`. Also exercises the self-connect rejection path.
//!
//! The test uses in-memory SQLite backends so it needs no filesystem access.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use ark_core::db::{init_schema, SqliteStorageBackend};
use ark_core::sync_client::SyncClient;
use ark_core::sync_server::{StorageBackend, SyncServer};
use ark_core::types::{PeerRecord, SyncEntity, VersionVector};
use rusqlite::Connection;
use serde_json::{json, Value};
use tokio::sync::Mutex;

fn make_storage(device_id: &str) -> Arc<SqliteStorageBackend> {
    let conn = Connection::open_in_memory().expect("in-memory db");
    init_schema(&conn).expect("schema");
    let shared = Arc::new(StdMutex::new(conn));
    let backend = Arc::new(SqliteStorageBackend::new(shared));
    backend.set_device_id(device_id);
    backend
}

fn todo_entity(id: &str, title: &str, device_id: &str, counter: u64) -> SyncEntity {
    let mut data = serde_json::Map::new();
    data.insert("title".to_string(), Value::String(title.to_string()));
    data.insert("notes".to_string(), Value::Null);
    data.insert("priority".to_string(), json!(0));
    data.insert("scheduledDate".to_string(), Value::Null);
    data.insert("deadline".to_string(), Value::Null);
    data.insert("reminderDate".to_string(), Value::Null);
    data.insert("isToday".to_string(), json!(false));
    data.insert("isEvening".to_string(), json!(false));
    data.insert("isSomeday".to_string(), json!(false));
    data.insert("isCompleted".to_string(), json!(false));
    data.insert("completedAt".to_string(), Value::Null);
    data.insert("isCancelled".to_string(), json!(false));
    data.insert("cancelledAt".to_string(), Value::Null);
    data.insert("isTrashed".to_string(), json!(false));
    data.insert("sortOrder".to_string(), json!(0));
    data.insert("headingId".to_string(), Value::Null);
    data.insert("projectId".to_string(), Value::Null);
    data.insert("areaId".to_string(), Value::Null);
    data.insert("tagIds".to_string(), json!([]));
    data.insert("checklistItems".to_string(), json!([]));
    data.insert("recurrenceRule".to_string(), Value::Null);
    data.insert("createdAt".to_string(), json!("2026-04-01T00:00:00.000Z"));

    SyncEntity {
        entity_type: "todo".to_string(),
        id: id.to_string(),
        data,
        hlc: format!("2026-04-01T00:00:00.000Z:{counter:06}:{device_id}"),
        deleted: None,
    }
}

async fn pick_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr.port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn round_trip_sync_between_two_servers() {
    // ---------------------------------------------------------------------
    // Server A — holds one todo already; Server B — empty.
    // After sync, B should contain the todo.
    // ---------------------------------------------------------------------
    let storage_a = make_storage("device-a");
    let storage_b = make_storage("device-b");

    // Seed one todo on side A.
    let seed = todo_entity("entity-a-1", "Seeded on A", "device-a", 1);
    storage_a.apply_entity(&seed).await;

    // Also stamp it into the local version vector so load_entities emits the
    // right HLC. Using the StorageBackend API via set_kv since the sync
    // server would normally do this.
    let mut vector_a: VersionVector = HashMap::new();
    vector_a.insert(seed.id.clone(), seed.hlc.clone());
    storage_a
        .set_kv(
            "lan_sync.version_vector",
            &serde_json::to_string(&vector_a).unwrap(),
        )
        .await;

    let server_a = Arc::new(SyncServer::new(storage_a.clone()
        as Arc<dyn StorageBackend>));
    let server_b = Arc::new(SyncServer::new(storage_b.clone()
        as Arc<dyn StorageBackend>));

    // Use two separate loopback ports.
    let port_a = pick_port().await;
    let port_b = pick_port().await;

    let server_a_changes: Arc<Mutex<Vec<SyncEntity>>> = Arc::new(Mutex::new(Vec::new()));
    let server_b_changes: Arc<Mutex<Vec<SyncEntity>>> = Arc::new(Mutex::new(Vec::new()));
    let server_a_peers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    {
        let changes = server_a_changes.clone();
        server_a
            .set_on_change(Arc::new(move |e| {
                let changes = changes.clone();
                tokio::spawn(async move {
                    changes.lock().await.push(e);
                });
            }))
            .await;
    }
    {
        let changes = server_b_changes.clone();
        server_b
            .set_on_change(Arc::new(move |e| {
                let changes = changes.clone();
                tokio::spawn(async move {
                    changes.lock().await.push(e);
                });
            }))
            .await;
    }
    {
        let peers = server_a_peers.clone();
        server_a
            .set_on_peer_connect(Arc::new(move |id| {
                let peers = peers.clone();
                tokio::spawn(async move {
                    peers.lock().await.push(id);
                });
            }))
            .await;
    }

    server_a
        .start_with_addr(
            "space-int",
            "device-a",
            Some("Alpha"),
            Some(vec![format!("127.0.0.1:{port_a}")]),
            &format!("127.0.0.1:{port_a}"),
        )
        .await
        .expect("server A start");

    server_b
        .start_with_addr(
            "space-int",
            "device-b",
            Some("Beta"),
            Some(vec![format!("127.0.0.1:{port_b}")]),
            &format!("127.0.0.1:{port_b}"),
        )
        .await
        .expect("server B start");

    // ---------------------------------------------------------------------
    // SyncClient on side B dialing server A
    // ---------------------------------------------------------------------
    let peer = PeerRecord {
        device_id: "device-a".to_string(),
        device_name: "Alpha".to_string(),
        addresses: vec![format!("127.0.0.1:{port_a}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
    };
    let client = Arc::new(SyncClient::new(
        storage_b.clone() as Arc<dyn StorageBackend>,
        peer,
        "device-b".to_string(),
        "Beta".to_string(),
        "space-int".to_string(),
        vec![format!("127.0.0.1:{port_b}")],
    ));
    client.start();

    // Wait for initial sync to settle.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Side B should now see the todo on its end.
    let loaded_b = storage_b.load_entities(&HashMap::new()).await;
    assert!(
        loaded_b.iter().any(|e| e.id == "entity-a-1"),
        "side B should have received seeded todo after initial sync; got {:?}",
        loaded_b.iter().map(|e| &e.id).collect::<Vec<_>>(),
    );

    // Side A peer-connect callback should have fired at least once.
    assert!(
        !server_a_peers.lock().await.is_empty(),
        "server A should have received a peer_connected event",
    );

    // ---------------------------------------------------------------------
    // Live change: broadcast from server A → should appear on B.
    // ---------------------------------------------------------------------
    let live = todo_entity("entity-a-2", "Live from A", "device-a", 2);
    storage_a.apply_entity(&live).await;
    server_a.broadcast_live_change(live.clone(), None).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let loaded_b_after = storage_b.load_entities(&HashMap::new()).await;
    assert!(
        loaded_b_after.iter().any(|e| e.id == "entity-a-2"),
        "side B should have received live change; got {:?}",
        loaded_b_after.iter().map(|e| &e.id).collect::<Vec<_>>(),
    );

    client.stop();
    server_a.stop().await;
    server_b.stop().await;

    println!("INTEGRATION OK");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_connect_is_rejected() {
    let storage = make_storage("device-self");
    let server = Arc::new(SyncServer::new(storage.clone() as Arc<dyn StorageBackend>));

    let port = pick_port().await;
    server
        .start_with_addr(
            "space-s",
            "device-self",
            Some("Self"),
            Some(vec![format!("127.0.0.1:{port}")]),
            &format!("127.0.0.1:{port}"),
        )
        .await
        .expect("server start");

    let peer = PeerRecord {
        device_id: "some-other-id".to_string(),
        device_name: "FakePeer".to_string(),
        addresses: vec![format!("127.0.0.1:{port}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
    };
    // The SyncClient will connect and send a hello with device_id = "device-self"
    // — same as the server. The server must reject the connection and NOT
    // emit a peer_connected event.
    let client = Arc::new(SyncClient::new(
        storage.clone() as Arc<dyn StorageBackend>,
        peer,
        "device-self".to_string(),
        "Self".to_string(),
        "space-s".to_string(),
        vec![format!("127.0.0.1:{port}")],
    ));

    let connect_counter = Arc::new(Mutex::new(0usize));
    {
        let counter = connect_counter.clone();
        server
            .set_on_peer_connect(Arc::new(move |_id| {
                let counter = counter.clone();
                tokio::spawn(async move {
                    *counter.lock().await += 1;
                });
            }))
            .await;
    }

    client.start();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let connected = server.connected_peer_count().await;
    assert_eq!(
        connected, 0,
        "server must have zero authenticated peers after self-connect",
    );
    assert_eq!(
        *connect_counter.lock().await,
        0,
        "server must not emit peer_connected for a self hello",
    );

    client.stop();
    server.stop().await;
}
