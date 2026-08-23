use ark_core::canonical_types::pending::{
    insert_pending_object, migrate_phase2_to_v3, replay_pending_for_type,
};
use ark_core::db::{init_schema, init_schema_prerequisites_for_phase3};
use ark_core::types::SyncEntity;
use rusqlite::{params, Connection};
use serde_json::json;

#[path = "support/phase3_legacy_fixtures.rs"]
mod phase3_legacy_fixtures;

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
         CREATE INDEX idx_sync_pending_awaited_type_version ON sync_pending_objects(awaited_type_id, awaited_type_version);",
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

    let row: (String, String, String, String, String) = conn.query_row(
        "SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM sync_pending_objects",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).unwrap();
    assert_eq!(
        row,
        (
            "o1".into(),
            payload.into(),
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
    let payload: String = conn.query_row("SELECT payload FROM sync_pending_objects WHERE id='o1' AND awaited_type_id='type-a' AND awaited_type_version='1.0.0'", [], |r| r.get(0)).unwrap();
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
    conn.execute("INSERT INTO object_type_versions (type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES ('future','1.0.0','{}','{}','{}','[]','{}','h1','now')", []).unwrap();
    let corrupted = serde_json::to_string(&entity_for_type(
        "o1",
        "wrong-type",
        "1.0.0",
        "2026-01-01T00:00:00Z:1",
        "bad",
    ))
    .unwrap();
    conn.execute(
        "INSERT INTO sync_pending_objects (id,payload,awaited_type_id,awaited_type_version,received_at) VALUES ('o1',?1,'future','1.0.0','received')",
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
    conn.execute("INSERT INTO object_type_versions (type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES ('future','1.0.0','{}','{}','{}','[]','{}','h1','now'),('future','2.0.0','{}','{}','{}','[]','{}','h2','now')", []).unwrap();
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
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM sync_pending_objects WHERE id='o1' AND awaited_type_version='2.0.0'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
}

#[test]
fn phase3_pending_rejects_inconsistent_payload_without_mutation() {
    let conn = phase2_pending_db();
    conn.execute("INSERT INTO sync_pending_objects VALUES ('o1','{\"id\":\"different\"}','future','1.0.0','r')", []).unwrap();
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

#[test]
fn phase3_pending_exact_v3_retry_is_noop() {
    let conn = phase2_pending_db();
    migrate_phase2_to_v3(&conn).unwrap();
    let before: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='sync_pending_objects'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    migrate_phase2_to_v3(&conn).unwrap();
    let after: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='sync_pending_objects'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn phase3_pending_exact_v3_retry_rejects_missing_index_and_malformed_row() {
    let conn = phase2_pending_db();
    let payload = serde_json::to_string(&entity("o1", "1.0.0", "h1", "x")).unwrap();
    conn.execute(
        "INSERT INTO sync_pending_objects VALUES (?1,?2,?3,?4,?5)",
        params!["o1", payload, "future", "1.0.0", "received"],
    )
    .unwrap();
    migrate_phase2_to_v3(&conn).unwrap();

    conn.execute_batch("DROP INDEX idx_sync_pending_awaited_type_version")
        .unwrap();
    assert!(migrate_phase2_to_v3(&conn).is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM sync_pending_objects", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );

    conn.execute_batch(
        "CREATE INDEX idx_sync_pending_awaited_type_version ON sync_pending_objects(awaited_type_id, awaited_type_version)",
    )
    .unwrap();
    conn.execute(
        "UPDATE sync_pending_objects SET payload='{\"id\":\"different\"}' WHERE id='o1'",
        [],
    )
    .unwrap();
    assert!(migrate_phase2_to_v3(&conn).is_err());
}

#[test]
fn phase3_pending_rebuild_failures_restore_phase2_table_and_rows() {
    fn assert_phase2_preserved(conn: &Connection, payload: &str) {
        let row: (String, String, String, String, String) = conn
            .query_row(
                "SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM sync_pending_objects",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "o1".into(),
                payload.into(),
                "future".into(),
                "1.0.0".into(),
                "received".into(),
            )
        );
        let pk: i64 = conn
            .query_row(
                "SELECT pk FROM pragma_table_info('sync_pending_objects') WHERE name='id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pk, 1);
    }

    for failure in ["create", "drop", "index"] {
        let conn = phase2_pending_db();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        let payload = serde_json::to_string(&entity("o1", "1.0.0", "h1", "x")).unwrap();
        conn.execute(
            "INSERT INTO sync_pending_objects VALUES (?1,?2,?3,?4,?5)",
            params!["o1", payload, "future", "1.0.0", "received"],
        )
        .unwrap();
        match failure {
            "create" => conn
                .execute_batch("CREATE TABLE sync_pending_objects_v3(blocker TEXT)")
                .unwrap(),
            "drop" => conn
                .execute_batch(
                    "CREATE TABLE pending_ref(id TEXT REFERENCES sync_pending_objects(id)); INSERT INTO pending_ref VALUES ('o1')",
                )
                .unwrap(),
            "index" => conn
                .execute_batch(
                    "DROP INDEX idx_sync_pending_awaited_type_version; CREATE TABLE index_name_owner(value TEXT); CREATE INDEX idx_sync_pending_awaited_type_version ON index_name_owner(value)",
                )
                .unwrap(),
            _ => unreachable!(),
        }

        assert!(
            migrate_phase2_to_v3(&conn).is_err(),
            "failure stage {failure}"
        );
        assert_phase2_preserved(&conn, &payload);
    }
}

#[test]
fn phase3_migration_creates_additive_state_and_promotes_registry_atomically() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM object_types", [], |row| row.get(0))
        .unwrap();

    let report = ark_core::canonical_types::migration::migrate_phase3(&conn).unwrap();
    assert_eq!(report.status, "completed");
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_types WHERE system_locked=1",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        9
    );
    assert_eq!(
        conn.query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias='task_obj'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "com.kosmos.task"
    );
    assert!(before >= 4);
    for table in [
        "object_local_state",
        "object_sync_versions",
        "object_migration_quarantine",
        "legacy_type_definition_archive",
        "canonical_migration_runs",
        "canonical_migration_items",
    ] {
        assert!(conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
    }
    let second = ark_core::canonical_types::migration::migrate_phase3(&conn).unwrap();
    assert_eq!(report.source_inventory_hash, second.source_inventory_hash);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM canonical_migration_runs", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        1
    );
}

#[test]
fn phase3_clear_all_removes_ephemeral_state_but_preserves_locked_registry_and_archive() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    phase3_legacy_fixtures::seed_historical_legacy_authorities(&conn);
    ark_core::canonical_types::migration::migrate_phase3(&conn).unwrap();
    conn.execute(
        "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES ('x','h',0)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO canonical_migration_items(contract_version,source_kind,source_id,source_hash,status,updated_at) VALUES ('phase3-canonical-v1','test','x','h','unchanged','now')", []).unwrap();
    ark_core::db::clear_all(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM object_sync_versions", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM canonical_migration_items", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM object_types WHERE system_locked=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        9
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM legacy_type_definition_archive",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        4
    );
}

#[test]
fn phase3_migration_collision_leaves_registry_and_ledger_unchanged() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute(
        "INSERT INTO object_type_aliases(alias,canonical_type_id,created_at) VALUES ('com.kosmos.task','com.kosmos.note','now')",
        [],
    )
    .unwrap();
    let snapshot = [
        ("object_types", "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked, owner_kind, owner_id, current_version, status, COALESCE(base_type_id, '') FROM object_types ORDER BY id"),
        ("object_type_versions", "SELECT type_id, version, schema_json, ui_schema_json, content_contract_json, relations_json, sync_policy_json, schema_hash, created_at FROM object_type_versions ORDER BY type_id, version"),
        ("object_type_aliases", "SELECT alias, canonical_type_id, created_at FROM object_type_aliases ORDER BY alias"),
    ]
    .into_iter()
    .map(|(name, sql)| {
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt
            .query_map([], |row| {
                let mut values = Vec::new();
                for index in 0..row.as_ref().column_count() {
                    values.push(format!("{:?}", row.get::<_, rusqlite::types::Value>(index)?));
                }
                Ok(values.join("\\u{1f}"))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (name, rows)
    })
    .collect::<Vec<_>>();
    let ledger_snapshot: Vec<(String, i64, String)> = conn
        .prepare("SELECT source_kind, attempt, checkpoint FROM canonical_migration_items ORDER BY source_kind, source_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(ark_core::canonical_types::migration::migrate_phase3(&conn).is_err());
    let after = [
        ("object_types", "SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked, owner_kind, owner_id, current_version, status, COALESCE(base_type_id, '') FROM object_types ORDER BY id"),
        ("object_type_versions", "SELECT type_id, version, schema_json, ui_schema_json, content_contract_json, relations_json, sync_policy_json, schema_hash, created_at FROM object_type_versions ORDER BY type_id, version"),
        ("object_type_aliases", "SELECT alias, canonical_type_id, created_at FROM object_type_aliases ORDER BY alias"),
    ]
    .into_iter()
    .map(|(name, sql)| {
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt
            .query_map([], |row| {
                let mut values = Vec::new();
                for index in 0..row.as_ref().column_count() {
                    values.push(format!("{:?}", row.get::<_, rusqlite::types::Value>(index)?));
                }
                Ok(values.join("\\u{1f}"))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (name, rows)
    })
    .collect::<Vec<_>>();
    let ledger_after: Vec<(String, i64, String)> = conn
        .prepare("SELECT source_kind, attempt, checkpoint FROM canonical_migration_items ORDER BY source_kind, source_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(snapshot, after);
    assert_eq!(ledger_snapshot, ledger_after);
    assert_eq!(
        conn.query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias='com.kosmos.task'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "com.kosmos.note"
    );
}
