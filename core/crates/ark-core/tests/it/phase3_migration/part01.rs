use ark_core::canonical_types::pending::{
    insert_pending_object, migrate_phase2_to_v3, replay_pending_for_type,
};
use ark_core::db::{init_schema, init_schema_prerequisites_for_phase3};
use ark_core::types::SyncEntity;
use rusqlite::{params, Connection};
use serde_json::json;

use crate::phase3_legacy_fixtures;

fn phase2_pending_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE sync_pending_objects (
            id TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            awaited_type_id TEXT NOT NULL,
            awaited_type_version TEXT NOT NULL,
            received_at TEXT NOT NULL
         );
         CREATE INDEX idx_sync_pending_awaited_type ON sync_pending_objects(awaited_type_id);
         CREATE INDEX idx_sync_pending_awaited_type_version ON \
         sync_pending_objects(awaited_type_id, awaited_type_version);",
    )
    .unwrap();
    conn
}

fn entity_for_type(
    id: &str,
    type_id: &str,
    type_version: &str,
    hlc: &str,
    title: &str,
) -> SyncEntity {
    SyncEntity {
        entity_type: "object".into(),
        id: id.into(),
        data: serde_json::Map::from_iter([
            ("id".into(), json!(id)),
            ("typeVersion".into(), json!(type_version)),
            ("typeId".into(), json!(type_id)),
            ("title".into(), json!(title)),
            ("contentJson".into(), json!({})),
            ("propsJson".into(), json!({})),
            ("createdAt".into(), json!("2026-01-01T00:00:00.000Z")),
            ("updatedAt".into(), json!("2026-01-01T00:00:00.000Z")),
        ]),
        hlc: hlc.into(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

fn entity(id: &str, type_version: &str, hlc: &str, title: &str) -> SyncEntity {
    entity_for_type(id, "future", type_version, hlc, title)
}

#[test]
fn phase3_pending_migrates_phase2_key_losslessly_and_preserves_five_columns() {
    let conn = phase2_pending_db();
    let payload = serde_json::to_string(&entity("o1", "1.0.0", "h1", "x")).unwrap();
    conn.execute(
        "INSERT INTO sync_pending_objects VALUES (?1,?2,?3,?4,?5)",
        params!["o1", payload, "future", "1.0.0", "received"],
    )
    .unwrap();

    migrate_phase2_to_v3(&conn).unwrap();

    let row: (String, String, String, String, String) = conn
        .query_row(
            concat!(
                "SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM ",
                "sync_pending_objects"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            "o1".into(),
            payload,
            "future".into(),
            "1.0.0".into(),
            "received".into()
        )
    );
    let pk: Vec<String> = conn
        .prepare("PRAGMA table_info(sync_pending_objects)")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, i64>(5)?, r.get::<_, String>(1)?)))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|(p, _)| *p > 0)
        .map(|(_, n)| n)
        .collect();
    assert_eq!(pk, vec!["id", "awaited_type_id", "awaited_type_version"]);
}

#[test]
fn phase3_pending_allows_distinct_awaited_pairs_for_same_object() {
    let conn = phase2_pending_db();
    migrate_phase2_to_v3(&conn).unwrap();
    insert_pending_object(
        &conn,
        &entity_for_type("o1", "type-a", "1.0.0", "2026-01-01T00:00:00Z:1", "a"),
        "type-a",
    )
    .unwrap();
    insert_pending_object(
        &conn,
        &entity_for_type("o1", "type-b", "2.0.0", "2026-01-01T00:00:00Z:1", "b"),
        "type-b",
    )
    .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sync_pending_objects WHERE id='o1'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}

#[test]
fn phase3_pending_replaces_only_exact_tuple_when_hlc_is_newer() {
    let conn = phase2_pending_db();
    migrate_phase2_to_v3(&conn).unwrap();
    insert_pending_object(
        &conn,
        &entity_for_type("o1", "type-a", "1.0.0", "2026-01-01T00:00:00Z:1", "old"),
        "type-a",
    )
    .unwrap();
    insert_pending_object(
        &conn,
        &entity_for_type("o1", "type-a", "1.0.0", "2026-01-01T00:00:00Z:2", "new"),
        "type-a",
    )
    .unwrap();
    insert_pending_object(
        &conn,
        &entity_for_type("o1", "type-b", "2.0.0", "2026-01-01T00:00:00Z:1", "other"),
        "type-b",
    )
    .unwrap();
    let payload: String = conn
        .query_row(
            concat!(
                "SELECT payload FROM sync_pending_objects WHERE id='o1' AND ",
                "awaited_type_id='type-a' AND awaited_type_version='1.0.0'"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(payload.contains("new"));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM sync_pending_objects", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn phase3_pending_insert_rejects_tuple_mismatch_without_mutation() {
    let conn = phase2_pending_db();
    migrate_phase2_to_v3(&conn).unwrap();
    let mismatched = entity_for_type(
        "o1",
        "payload-type",
        "1.0.0",
        "2026-01-01T00:00:00Z:1",
        "bad",
    );

    assert!(insert_pending_object(&conn, &mismatched, "awaited-type").is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM sync_pending_objects", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
}

#[test]
fn phase3_pending_replay_rejects_corrupted_tuple_atomically() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    migrate_phase2_to_v3(&conn).unwrap();
    let typ = ark_core::types::ObjectType {
        id: "future".into(),
        name: "Future".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    ark_core::db::upsert_object_type(&conn, &typ).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_type_versions (type_id,version,schema_json,",
            "ui_schema_json,content_contract_json,relations_json,sync_policy_json,",
            "schema_hash,created_at) VALUES ('future','1.0.0','{}','{}','{}','[]','{}',",
            "'h1','now')"
        ),
        [],
    )
    .unwrap();
    let corrupted = serde_json::to_string(&entity_for_type(
        "o1",
        "wrong-type",
        "1.0.0",
        "2026-01-01T00:00:00Z:1",
        "bad",
    ))
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO sync_pending_objects (id,payload,awaited_type_id,",
            "awaited_type_version,received_at) VALUES ('o1',?1,'future','1.0.0',",
            "'received')"
        ),
        params![corrupted],
    )
    .unwrap();

    assert!(replay_pending_for_type(&conn, "future", "1.0.0").is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM sync_pending_objects", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        1
    );
    assert!(ark_core::db::get_object(&conn, "o1").unwrap().is_none());
}

#[test]
fn phase3_pending_replay_and_delete_are_exact_tuple_isolated() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    migrate_phase2_to_v3(&conn).unwrap();
    let typ = ark_core::types::ObjectType {
        id: "future".into(),
        name: "Future".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    ark_core::db::upsert_object_type(&conn, &typ).unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_type_versions (type_id,version,schema_json,",
            "ui_schema_json,content_contract_json,relations_json,sync_policy_json,",
            "schema_hash,created_at) VALUES ('future','1.0.0','{}','{}','{}','[]','{}',",
            "'h1','now'),('future','2.0.0','{}','{}','{}','[]','{}','h2','now')"
        ),
        [],
    )
    .unwrap();
    insert_pending_object(
        &conn,
        &entity("o1", "1.0.0", "2026-01-01T00:00:00Z:1", "a"),
        "future",
    )
    .unwrap();
    insert_pending_object(
        &conn,
        &entity("o1", "2.0.0", "2026-01-01T00:00:00Z:1", "b"),
        "future",
    )
    .unwrap();
    assert_eq!(
        replay_pending_for_type(&conn, "future", "1.0.0").unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sync_pending_objects WHERE id='o1' AND
             awaited_type_version='2.0.0'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn phase3_pending_rejects_inconsistent_payload_without_mutation() {
    let conn = phase2_pending_db();
    conn.execute(
        concat!(
            "INSERT INTO sync_pending_objects VALUES ('o1','{\"id\":\"different\"}',",
            "'future','1.0.0','r')"
        ),
        [],
    )
    .unwrap();
    assert!(migrate_phase2_to_v3(&conn).is_err());
    let pk: i64 = conn
        .query_row(
            "SELECT pk FROM pragma_table_info('sync_pending_objects') WHERE name='id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pk, 1);
}
