#![allow(clippy::unwrap_used)]

use ark_domain::types::{TrackedApp, UsageSession, VersionVector};
use ark_store::open::init_schema;
use ark_store::usage_sync_log::{compact_usage_sync_log, usage_log_complete_through};
use rusqlite::Connection;
use serde_json::json;

fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
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
    ark_store::sync_kv::get_sync_kv(connection, ark_store::VERSION_VECTOR_KEY)
        .unwrap_or(None)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_stored_vector(connection: &Connection, vector: &VersionVector) {
    ark_store::sync_kv::set_sync_kv(
        connection,
        ark_store::VERSION_VECTOR_KEY,
        &serde_json::to_string(vector).unwrap(),
    )
    .unwrap();
}

fn usage_cursor(connection: &Connection, device_id: &str) -> u64 {
    stored_vector(connection)
        .get(&format!("@usage:{device_id}"))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

fn count_usage_entities(connection: &Connection) -> i64 {
    connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM usage_sessions) +
                    (SELECT COUNT(*) FROM usage_events) +
                    (SELECT COUNT(*) FROM usage_days)",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn sync_entity_page(
    connection: &Connection,
    vector: &VersionVector,
    device_id: &str,
    offset: usize,
    limit: usize,
) -> Vec<ark_domain::types::SyncEntity> {
    ark_store::sync_entities::collect_entities_page(connection, vector, device_id, offset, limit)
        .unwrap()
}

fn apply_entity(connection: &Connection, entity: &ark_domain::types::SyncEntity) {
    let transaction = connection.unchecked_transaction().unwrap();
    ark_store::sync_entities::apply_inbound_entity(&transaction, entity).unwrap();
    transaction.commit().unwrap();
}

/// Simulates one VersionVector pull: the server pages Store entities against
/// the peer's vector, the receiver applies them, and the final complete-through
/// claim advances usage cursors. `claims = false` models an older protocol peer.
fn pull_usage(
    server: &Connection,
    server_device: &str,
    client: &Connection,
    claims: bool,
) -> usize {
    let through = claims.then(|| usage_log_complete_through(server, None).unwrap());
    let remote = stored_vector(client);
    let mut offset = 0;
    let mut applied = 0;
    loop {
        let entities = sync_entity_page(server, &remote, server_device, offset, 3);
        if entities.is_empty() {
            break;
        }
        offset += entities.len();
        for entity in entities {
            let is_usage = ark_domain::types::is_sequenced_usage_entity(&entity.entity_type);
            apply_entity(client, &entity);
            applied += usize::from(is_usage);
        }
    }
    if let Some(through) = through {
        let mut vector = stored_vector(client);
        ark_sync::protocol::apply_usage_complete_through(&mut vector, &through);
        save_stored_vector(client, &vector);
    }
    applied
}

fn compacted_usage_server() -> Connection {
    let connection = setup_db();
    ark_store::usage::upsert_tracked_app(&connection, &make_tracked_app("app-1")).unwrap();
    for id in ["s-1", "s-2", "s-3"] {
        ark_store::usage::upsert_usage_session(&connection, &make_usage_session(id, "app-1"))
            .unwrap();
    }
    // Do not backfill fresh HLC refs before inserting the churn fixture.
    let hlc = |seq: u64| format!("2026-08-01T00:00:{seq:02}.000Z:{seq:06}:dev-a");
    for (seq, id) in [
        (1, "s-1"),
        (2, "s-1"),
        (3, "s-2"),
        (4, "s-1"),
        (5, "s-2"),
        (6, "s-3"),
    ] {
        ark_store::usage_sync_log::record_usage_sequence(
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
            1_000
        )
        .unwrap(),
        3,
        "fixture must compact refs 1, 2 and 3"
    );
    connection
}

#[test]
fn fresh_device_converges_past_compacted_holes() {
    let server = compacted_usage_server();
    let client = setup_db();
    ark_store::usage::upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

    assert_eq!(pull_usage(&server, "dev-a", &client, true), 3);
    assert_eq!(count_usage_entities(&client), 3);
    assert_eq!(usage_cursor(&client, "dev-a"), 6);
    assert_eq!(pull_usage(&server, "dev-a", &client, true), 0);
}

#[test]
fn complete_through_does_not_jump_mid_page() {
    let server = compacted_usage_server();
    let client = setup_db();
    ark_store::usage::upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

    let remote = stored_vector(&client);
    let first_page = sync_entity_page(&server, &remote, "dev-a", 0, 2);
    assert_eq!(first_page.len(), 2);
    for entity in &first_page {
        apply_entity(&client, entity);
    }
    assert_eq!(
        usage_cursor(&client, "dev-a"),
        0,
        "cursor must not jump: seqs 1..3 are compacted holes on the receiver"
    );

    let through = usage_log_complete_through(&server, None).unwrap();
    let mut vector = stored_vector(&client);
    ark_sync::protocol::apply_usage_complete_through(&mut vector, &through);
    save_stored_vector(&client, &vector);
    assert_eq!(usage_cursor(&client, "dev-a"), 6);
}

#[test]
fn relay_converges_with_compaction_on_both_hops() {
    let server_a = compacted_usage_server();
    let relay_b = setup_db();
    ark_store::usage::upsert_tracked_app(&relay_b, &make_tracked_app("app-1")).unwrap();
    pull_usage(&server_a, "dev-a", &relay_b, true);
    assert_eq!(usage_cursor(&relay_b, "dev-a"), 6);

    ark_store::usage::upsert_usage_session(&relay_b, &make_usage_session("s-b1", "app-1")).unwrap();
    for (seq, hlc) in [
        (1, "2026-08-01T00:00:01.000Z:000001:dev-b"),
        (2, "2026-08-01T00:00:02.000Z:000002:dev-b"),
    ] {
        ark_store::usage_sync_log::record_usage_sequence(
            &relay_b,
            "usage_session",
            "s-b1",
            "dev-b",
            seq,
            hlc,
            false,
        )
        .unwrap();
    }
    assert_eq!(
        compact_usage_sync_log(&relay_b, Some("dev-b"), "2026-09-01T00:00:00.000Z", 1_000).unwrap(),
        1
    );

    let client_c = setup_db();
    ark_store::usage::upsert_tracked_app(&client_c, &make_tracked_app("app-1")).unwrap();
    pull_usage(&relay_b, "dev-b", &client_c, true);
    assert_eq!(count_usage_entities(&client_c), 4);
    assert_eq!(usage_cursor(&client_c, "dev-a"), 6);
    assert_eq!(usage_cursor(&client_c, "dev-b"), 2);
    assert_eq!(pull_usage(&relay_b, "dev-b", &client_c, true), 0);
}

#[test]
fn old_protocol_peer_still_converges_without_claims() {
    let server = compacted_usage_server();
    let client = setup_db();
    ark_store::usage::upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

    assert_eq!(pull_usage(&server, "dev-a", &client, false), 3);
    assert_eq!(count_usage_entities(&client), 3);
    assert_eq!(
        usage_cursor(&client, "dev-a"),
        0,
        "contiguous cursor stops at the first hole — pre-fix behaviour"
    );
    assert_eq!(pull_usage(&server, "dev-a", &client, false), 3);
}

#[test]
fn complete_through_snapshot_excludes_mid_stream_appends() {
    let server = compacted_usage_server();
    let client = setup_db();
    ark_store::usage::upsert_tracked_app(&client, &make_tracked_app("app-1")).unwrap();

    let through = usage_log_complete_through(&server, None).unwrap();
    assert_eq!(through["dev-a"], 6);

    let remote = stored_vector(&client);
    let mut offset = 0;
    let mut appended = false;
    loop {
        let entities = sync_entity_page(&server, &remote, "dev-a", offset, 3);
        if entities.is_empty() {
            break;
        }
        offset += entities.len();
        for entity in entities {
            apply_entity(&client, &entity);
        }
        if !appended {
            appended = true;
            ark_store::usage::upsert_usage_session(&server, &make_usage_session("s-4", "app-1"))
                .unwrap();
            ark_store::usage_sync_log::record_usage_sequence(
                &server,
                "usage_session",
                "s-4",
                "dev-a",
                7,
                "2026-10-01T00:00:07.000Z:000007:dev-a",
                false,
            )
            .unwrap();
        }
    }

    let mut vector = stored_vector(&client);
    ark_sync::protocol::apply_usage_complete_through(&mut vector, &through);
    save_stored_vector(&client, &vector);
    assert_eq!(usage_cursor(&client, "dev-a"), 6);
    assert!(pull_usage(&server, "dev-a", &client, true) >= 1);
    assert_eq!(usage_cursor(&client, "dev-a"), 7);
}
