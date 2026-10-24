#![allow(clippy::unwrap_used)]

use ark_domain::types::{SyncEntity, TrackedApp, UsageSession, VersionVector};
use rusqlite::Connection;
use serde_json::{json, Value};

use super::SqliteWriter;

fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}

fn sync_entity<T: serde::Serialize>(entity_type: &str, id: &str, value: &T) -> SyncEntity {
    let Value::Object(mut data) = serde_json::to_value(value).unwrap() else {
        panic!("expected an object payload");
    };
    data.remove("id");
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc: "2026-01-01T00:00:00.000Z:000001:peer-a".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

fn tracked_app_entity(id: &str) -> SyncEntity {
    sync_entity("tracked_app", id, &tracked_app(id))
}

fn usage_session_entity(id: &str, tracked_app_id: &str, origin: &str, seq: u64) -> SyncEntity {
    let mut entity = sync_entity("usage_session", id, &usage_session(id, tracked_app_id));
    entity.hlc = format!("2026-01-01T00:00:00.000Z:{seq:06}:{origin}");
    entity.origin_device_id = Some(origin.to_string());
    entity.origin_seq = Some(seq);
    entity
}

fn tracked_app(id: &str) -> TrackedApp {
    TrackedApp {
        id: id.to_string(),
        platform: "windows".to_string(),
        exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
        normalized_exe_path: r"c:\\apps\\demo\\demo.exe".to_string(),
        process_name: "demo.exe".to_string(),
        display_name: Some("Demo App".to_string()),
        publisher: Some("Demo Corp".to_string()),
        icon_ref: None,
        first_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
        last_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
    }
}

fn usage_session(id: &str, tracked_app_id: &str) -> UsageSession {
    UsageSession {
        id: id.to_string(),
        tracked_app_id: tracked_app_id.to_string(),
        device_id: "device-1".to_string(),
        device_name: "Test Device".to_string(),
        platform: "windows".to_string(),
        started_at: "2026-01-01T00:00:00.000Z".to_string(),
        ended_at: Some("2026-01-01T00:10:00.000Z".to_string()),
        runtime_ms: 600_000,
        foreground_ms: 600_000,
        idle_ms: 0,
        window_title: Some("Demo Window".to_string()),
        process_name: "demo.exe".to_string(),
        exe_path: r"C:\\Apps\\Demo\\demo.exe".to_string(),
        pid_start: Some(1234),
        pid_end: Some(1234),
        meta_json: json!({"note": "session"}),
    }
}

fn apply_entity(connection: &mut Connection, entity: &SyncEntity) {
    let transaction = connection.transaction().unwrap();
    crate::sync_entities::apply_inbound_entity(&transaction, entity).unwrap();
    transaction.commit().unwrap();
}

#[test]
fn usage_sync_page_does_not_duplicate_entities_behind_dead_refs() {
    // A usage_sync_log ref whose entity row is gone must not corrupt the
    // page offsets: callers advance `offset` by entities returned, while
    // the SQL OFFSET counts refs. A skipped ref made the next page
    // re-emit entities it had already returned.
    let mut connection = setup_db();
    apply_entity(&mut connection, &tracked_app_entity("app-drift"));
    apply_entity(
        &mut connection,
        &usage_session_entity("session-dead", "app-drift", "peer-a", 2),
    );
    apply_entity(
        &mut connection,
        &usage_session_entity("session-live", "app-drift", "peer-a", 3),
    );

    // Simulate a dead ref: row gone with no tombstone (e.g. clear_all
    // residue or a delete path that bypassed sync bookkeeping).
    connection
        .execute("DELETE FROM usage_sessions WHERE id = 'session-dead'", [])
        .unwrap();

    let loaded =
        crate::sync_entities::collect_entities(&connection, &VersionVector::new(), "device")
            .unwrap();
    let live_count = loaded
        .iter()
        .filter(|entity| entity.id == "session-live")
        .count();
    assert_eq!(
        live_count, 1,
        "dead ref must not cause session-live to be emitted twice: {loaded:?}"
    );
    // The dead ref must still surface as a tombstone so the receiver's
    // contiguous usage cursor can advance past its seq.
    let tombstones = loaded
        .iter()
        .filter(|entity| entity.id == "session-dead" && entity.deleted == Some(true))
        .count();
    assert_eq!(
        tombstones, 1,
        "dead ref should surface exactly one tombstone entity: {loaded:?}"
    );
}

#[test]
fn clear_all_clears_usage_sync_log_residue() {
    // clear_all wiped rows + tombstones but left usage_sync_log /
    // usage_sync_versions behind: dead refs then re-emitted stale
    // tombstones / skipped rows to every fresh peer, and the leftover
    // versions blocked peers from re-sending resurrected entities.
    let mut connection = setup_db();
    apply_entity(&mut connection, &tracked_app_entity("app-clear"));
    apply_entity(
        &mut connection,
        &usage_session_entity("session-clear", "app-clear", "peer-a", 7),
    );
    let mut writer = SqliteWriter::from_connection(connection);
    writer.with_connection_mut(|connection| {
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "applied usage entity should have a sync-log ref");
    });

    writer.clear_all().unwrap();
    writer.with_connection_mut(|connection| {
        for table in ["usage_sync_log", "usage_sync_versions"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "clear_all must clear {table}");
        }
    });
}
