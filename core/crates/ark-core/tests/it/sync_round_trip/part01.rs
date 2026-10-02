// #![allow(clippy::unwrap_used)]

// End-to-end sync integration test (AC20 / AC18 / AC19).
//
// Spins up two `SyncServer` instances in-process on loopback, dials one from
// the other via `SyncClient`, and walks through a full protocol exchange:
// `hello` → `version_vector` → `sync_changes` + ACK → `live_change` +
// `live_ack`. Also exercises the self-connect rejection path.
//
// The test uses in-memory SQLite backends so it needs no filesystem access.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use ark_core::db::{init_schema, SqliteStorageBackend};
use ark_core::sync_client::SyncClient;
use ark_core::sync_server::{StorageBackend, SyncServer};
use ark_core::types::{
    PeerRecord, SyncEntity, TrackedApp, UsageEvent, UsageSession, VersionVector,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tokio::sync::Mutex;

fn make_storage(device_id: &str) -> Arc<SqliteStorageBackend> {
    let conn = Connection::open_in_memory().expect("in-memory db");
    init_schema(&conn).expect("schema");
    let shared = Arc::new(StdMutex::new(conn));
    let backend = Arc::new(SqliteStorageBackend::new(shared));
    backend.set_device_id(device_id).unwrap();
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
        origin_device_id: None,
        origin_seq: None,
    }
}

fn sync_entity_from_value(
    entity_type: &str,
    id: &str,
    value: Value,
    device_id: &str,
    counter: u64,
) -> SyncEntity {
    let data = value
        .as_object()
        .cloned()
        .expect("usage sync entity value should be an object");
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc: format!("2026-04-01T00:00:00.000Z:{counter:06}:{device_id}"),
        deleted: None,
        origin_device_id: ark_core::db::is_sequenced_usage_entity(entity_type)
            .then(|| device_id.to_string()),
        origin_seq: ark_core::db::is_sequenced_usage_entity(entity_type).then_some(counter),
    }
}

fn tracked_app_entity(id: &str, device_id: &str, counter: u64) -> SyncEntity {
    let tracked_app = TrackedApp {
        id: id.to_string(),
        platform: "windows".to_string(),
        exe_path: r"C:\\Games\\Atlas\\atlas.exe".to_string(),
        normalized_exe_path: r"c:\\games\\atlas\\atlas.exe".to_string(),
        process_name: "atlas.exe".to_string(),
        display_name: Some("Atlas".to_string()),
        publisher: Some("Mundus".to_string()),
        icon_ref: None,
        first_seen_at: "2026-04-01T00:00:00.000Z".to_string(),
        last_seen_at: "2026-04-01T00:05:00.000Z".to_string(),
    };
    sync_entity_from_value(
        "tracked_app",
        &tracked_app.id,
        serde_json::to_value(&tracked_app).unwrap(),
        device_id,
        counter,
    )
}

fn usage_session_entity(
    id: &str,
    tracked_app_id: &str,
    device_id: &str,
    counter: u64,
) -> SyncEntity {
    let session = UsageSession {
        id: id.to_string(),
        tracked_app_id: tracked_app_id.to_string(),
        device_id: device_id.to_string(),
        device_name: "Alpha".to_string(),
        platform: "windows".to_string(),
        started_at: "2026-04-01T00:00:00.000Z".to_string(),
        ended_at: Some("2026-04-01T00:45:00.000Z".to_string()),
        runtime_ms: 2_700_000,
        foreground_ms: 2_400_000,
        idle_ms: 300_000,
        window_title: Some("Atlas Launcher".to_string()),
        process_name: "atlas.exe".to_string(),
        exe_path: r"C:\\Games\\Atlas\\atlas.exe".to_string(),
        pid_start: Some(501),
        pid_end: Some(501),
        meta_json: json!({"source": "integration-test"}),
    };
    sync_entity_from_value(
        "usage_session",
        &session.id,
        serde_json::to_value(&session).unwrap(),
        device_id,
        counter,
    )
}

fn usage_event_entity(
    id: &str,
    tracked_app_id: &str,
    usage_session_id: &str,
    device_id: &str,
    counter: u64,
) -> SyncEntity {
    let event = UsageEvent {
        id: id.to_string(),
        tracked_app_id: tracked_app_id.to_string(),
        usage_session_id: Some(usage_session_id.to_string()),
        device_id: device_id.to_string(),
        device_name: "Alpha".to_string(),
        platform: "windows".to_string(),
        occurred_at: "2026-04-01T00:15:00.000Z".to_string(),
        kind: "window_changed".to_string(),
        window_title: Some("Atlas Match".to_string()),
        process_name: "atlas.exe".to_string(),
        exe_path: r"C:\\Games\\Atlas\\atlas.exe".to_string(),
        pid: Some(501),
        is_foreground: true,
        is_idle: false,
        meta_json: json!({"source": "integration-test", "windowTitle": "Atlas Match"}),
    };
    sync_entity_from_value(
        "usage_event",
        &event.id,
        serde_json::to_value(&event).unwrap(),
        device_id,
        counter,
    )
}

async fn pick_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr.port()
}
