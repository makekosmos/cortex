#![allow(clippy::unwrap_used)]

use ark_domain::types::{TrackedApp, UsageSession, VersionVector};
use rusqlite::Connection;
use serde_json::json;

use super::{compact_usage_sync_log, usage_log_complete_through};

fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}

fn make_tracked_app(id: &str) -> TrackedApp {
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

fn make_usage_session(id: &str, tracked_app_id: &str) -> UsageSession {
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

fn stored_vector(connection: &Connection) -> VersionVector {
    crate::sync_kv::get_sync_kv(connection, crate::VERSION_VECTOR_KEY)
        .unwrap_or(None)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_stored_vector(connection: &Connection, vector: &VersionVector) {
    crate::sync_kv::set_sync_kv(
        connection,
        crate::VERSION_VECTOR_KEY,
        &serde_json::to_string(vector).unwrap(),
    )
    .unwrap();
}

fn usage_log_refs(connection: &Connection) -> Vec<(String, i64, String, String)> {
    connection
        .prepare(
            "SELECT device_id, seq, entity_type, entity_id
             FROM usage_sync_log ORDER BY device_id, seq",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn compacted_usage_server() -> Connection {
    let connection = setup_db();
    crate::usage::upsert_tracked_app(&connection, &make_tracked_app("app-1")).unwrap();
    for id in ["s-1", "s-2", "s-3"] {
        crate::usage::upsert_usage_session(&connection, &make_usage_session(id, "app-1")).unwrap();
    }
    let hlc = |seq: u64| format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev-a");
    for (seq, id) in [
        (1, "s-1"),
        (2, "s-1"),
        (3, "s-2"),
        (4, "s-1"),
        (5, "s-2"),
        (6, "s-3"),
    ] {
        crate::usage_sync_log::record_usage_sequence(
            &connection,
            "usage_session",
            id,
            "dev-a",
            seq,
            &hlc(seq),
            false,
        )
        .unwrap();
    }
    assert_eq!(
        compact_usage_sync_log(
            &connection,
            Some("dev-a"),
            "2026-09-01T00:00:00.000Z",
            1_000,
        )
        .unwrap(),
        3,
        "fixture must compact refs 1, 2 and 3"
    );
    connection
}

#[test]
fn compaction_never_deletes_above_foreign_contiguous_cursor() {
    // Relay safety: refs whose seq is above our contiguous coverage of
    // that origin are gaps we cannot vouch for — superseded or not, they stay.
    let connection = setup_db();
    let insert = |seq: i64, entity: &str, hlc: &str| {
        connection
            .execute(
                "INSERT INTO usage_sync_log (device_id, seq, entity_type, entity_id, hlc, deleted)
                 VALUES ('remote', ?1, 'usage_session', ?2, ?3, 0)",
                rusqlite::params![seq, entity, hlc],
            )
            .unwrap();
    };
    insert(1, "s-1", "2026-08-01T00:00:01.000Z:000001:remote");
    insert(2, "s-1", "2026-08-01T00:00:02.000Z:000002:remote");
    insert(4, "s-2", "2026-08-01T00:00:04.000Z:000004:remote");
    insert(5, "s-2", "2026-08-01T00:00:05.000Z:000005:remote");
    connection
        .execute(
            "INSERT INTO usage_sync_heads (device_id, max_seq) VALUES ('remote', 5)",
            [],
        )
        .unwrap();
    let mut vector = VersionVector::new();
    vector.insert("@usage:remote".to_string(), "2".to_string());
    save_stored_vector(&connection, &vector);

    let deleted = compact_usage_sync_log(
        &connection,
        Some("dev-a"),
        "2026-09-01T00:00:00.000Z",
        1_000,
    )
    .unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(
        usage_log_refs(&connection)
            .iter()
            .map(|(_, seq, _, _)| *seq)
            .collect::<Vec<_>>(),
        vec![2, 4, 5],
        "seq 4 is superseded but above our contiguous cursor — it must stay"
    );
}

#[test]
fn complete_through_reports_cursors_and_the_own_head() {
    let connection = compacted_usage_server();
    assert_eq!(
        usage_log_complete_through(&connection, None).unwrap()["dev-a"],
        6
    );

    let connection = setup_db();
    crate::usage::upsert_tracked_app(&connection, &make_tracked_app("app-1")).unwrap();
    crate::usage::upsert_usage_session(&connection, &make_usage_session("s-1", "app-1")).unwrap();
    for seq in 1..=3 {
        crate::usage_sync_log::record_usage_sequence(
            &connection,
            "usage_session",
            "s-1",
            "dev",
            seq,
            &format!("2026-08-01T00:00:0{seq}.000Z:{seq:06}:dev"),
            false,
        )
        .unwrap();
    }
    let mut vector = stored_vector(&connection);
    vector.remove("@usage:dev");
    save_stored_vector(&connection, &vector);
    assert_eq!(
        usage_log_complete_through(&connection, None).unwrap()["dev"],
        0
    );
    assert_eq!(
        usage_log_complete_through(&connection, Some("dev")).unwrap()["dev"],
        3
    );

    let connection = setup_db();
    connection
        .execute(
            "INSERT INTO usage_sync_heads (device_id, max_seq) VALUES ('remote', 9)",
            [],
        )
        .unwrap();
    let mut vector = VersionVector::new();
    vector.insert("@usage:remote".to_string(), "4".to_string());
    save_stored_vector(&connection, &vector);
    assert_eq!(
        usage_log_complete_through(&connection, None).unwrap()["remote"],
        4
    );
    assert_eq!(
        usage_log_complete_through(&connection, Some("me")).unwrap()["remote"],
        4
    );
}

#[test]
fn own_refs_compact_by_journal_head_when_cursor_is_frozen() {
    let connection = setup_db();
    crate::usage::upsert_tracked_app(&connection, &make_tracked_app("app-1")).unwrap();
    crate::usage::upsert_usage_session(&connection, &make_usage_session("s-1", "app-1")).unwrap();
    for seq in 1..=3 {
        crate::usage_sync_log::record_usage_sequence(
            &connection,
            "usage_session",
            "s-1",
            "dev",
            seq,
            &format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev"),
            false,
        )
        .unwrap();
    }
    let mut vector = stored_vector(&connection);
    vector.remove("@usage:dev");
    save_stored_vector(&connection, &vector);

    assert_eq!(
        compact_usage_sync_log(&connection, Some("dev"), "2026-09-01T00:00:00.000Z", 100).unwrap(),
        2,
        "own head bounds superseded own refs"
    );
    assert_eq!(usage_log_refs(&connection).len(), 1, "the newest ref stays");
}

#[test]
fn compaction_with_no_own_device_binds_everything_to_cursors() {
    let connection = setup_db();
    crate::usage::upsert_tracked_app(&connection, &make_tracked_app("app-1")).unwrap();
    crate::usage::upsert_usage_session(&connection, &make_usage_session("s-1", "app-1")).unwrap();
    for seq in 1..=3 {
        crate::usage_sync_log::record_usage_sequence(
            &connection,
            "usage_session",
            "s-1",
            "dev",
            seq,
            &format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev"),
            false,
        )
        .unwrap();
    }
    let mut vector = stored_vector(&connection);
    vector.remove("@usage:dev");
    save_stored_vector(&connection, &vector);

    assert_eq!(
        compact_usage_sync_log(&connection, None, "2026-09-01T00:00:00.000Z", 100).unwrap(),
        0
    );
    assert_eq!(usage_log_refs(&connection).len(), 3);
}
