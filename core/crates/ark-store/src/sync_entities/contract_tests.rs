#![allow(clippy::unwrap_used)]

use ark_domain::types::{ArkObject, VersionVector};
use rusqlite::Connection;
use serde_json::json;

fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}

fn seed_object(connection: &Connection, id: &str, type_id: &str, title: &str, hlc: &str) {
    let object = ArkObject {
        id: id.to_owned(),
        type_id: type_id.to_owned(),
        type_version: "0.1.0".to_owned(),
        title: title.to_owned(),
        content_json: json!({}),
        props_json: json!({}),
        created_at: "2026-10-15T00:00:00.000Z".to_owned(),
        updated_at: "2026-10-15T00:00:00.000Z".to_owned(),
        deleted_at: None,
    };
    crate::objects::upsert_object_compat(connection, &object).unwrap();
    crate::sync_versions::record_remote_object_revision(connection, id, hlc, false).unwrap();
}

#[test]
fn initial_sync_snapshot_omits_tasks_owned_by_the_merge_journal() {
    let connection = setup_db();
    seed_object(
        &connection,
        "snapshot-merge-task",
        "core.task",
        "Must use the task merge journal",
        "2026-10-15T00:00:00.000Z:000011:source-device",
    );
    seed_object(
        &connection,
        "snapshot-note",
        "core.note",
        "Still uses the ordinary feed",
        "2026-10-15T00:00:00.000Z:000012:source-device",
    );
    let vector = VersionVector::default();

    let before =
        crate::sync_entities::collect_entities_page(&connection, &vector, "device", 0, 16).unwrap();
    assert!(before
        .iter()
        .any(|entity| entity.id == "snapshot-merge-task"));
    assert!(before.iter().any(|entity| entity.id == "snapshot-note"));

    connection
        .execute(
            "INSERT INTO task_merge_bootstraps(
                 bootstrap_id, object_id, bootstrap_json, payload_sha256
             ) VALUES ('00000000000000000000000000000001', ?1, '{}', 'digest')",
            ["snapshot-merge-task"],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sync_tombstones(id, entity_type, hlc, deleted_at, type_id)
             VALUES (?1, 'object', 'r2', '2026-10-15T00:00:01Z', 'core.task')",
            ["snapshot-merge-task"],
        )
        .unwrap();

    let after =
        crate::sync_entities::collect_entities_page(&connection, &vector, "device", 0, 16).unwrap();
    assert!(after
        .iter()
        .all(|entity| entity.id != "snapshot-merge-task"));
    assert!(after.iter().any(|entity| entity.id == "snapshot-note"));
}

#[test]
fn initial_sync_snapshot_fails_when_merge_mode_cannot_be_checked() {
    let connection = setup_db();
    connection
        .execute_batch("DROP TABLE task_merge_bootstraps")
        .unwrap();

    let error = crate::sync_entities::collect_entities_page(
        &connection,
        &VersionVector::default(),
        "device",
        0,
        16,
    )
    .unwrap_err();
    assert!(error.contains("task_merge_bootstraps"));
}
