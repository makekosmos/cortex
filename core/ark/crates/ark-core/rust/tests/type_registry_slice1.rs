use ark_core::db::{init_schema, init_schema_prerequisites_for_phase3};
use ark_core::type_registry::{
    canonical_schema_hash, insert_type_version, list_type_versions, register_alias, register_type,
    resolve_alias, set_base_type, set_current_version, set_status, AliasRecord, TypeRegistration,
    TypeVersion,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

fn legacy_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"PRAGMA foreign_keys = ON;
         CREATE TABLE object_types (
           id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL,
           ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
           system_locked INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE objects (
           id TEXT PRIMARY KEY, type_id TEXT NOT NULL, title TEXT NOT NULL,
           content_json TEXT NOT NULL, props_json TEXT NOT NULL, created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL, deleted_at TEXT,
           FOREIGN KEY(type_id) REFERENCES object_types(id)
         );
         CREATE TABLE object_links (
           id TEXT PRIMARY KEY, source_object_id TEXT NOT NULL, target_object_id TEXT NOT NULL,
           link_type TEXT NOT NULL, created_at TEXT NOT NULL,
           FOREIGN KEY(source_object_id) REFERENCES objects(id),
           FOREIGN KEY(target_object_id) REFERENCES objects(id)
         );
         CREATE TABLE sync_pending_objects (
           id TEXT PRIMARY KEY, payload TEXT NOT NULL, awaited_type_id TEXT NOT NULL,
           received_at TEXT NOT NULL
         );
         INSERT INTO object_types VALUES ('legacy', 'Legacy', '{"b":2,"a":1}', '{"ui":true}', 'created', 'updated', 0);
         INSERT INTO objects VALUES ('live', 'legacy', 'Live', '{"x":1}', '{"p":2}', 'oc', 'ou', NULL);
         INSERT INTO objects VALUES ('deleted', 'legacy', 'Deleted', '{"x":2}', '{"p":3}', 'dc', 'du', 'deleted-at');
         INSERT INTO object_links VALUES ('link', 'live', 'deleted', 'related', 'link-time');
         INSERT INTO sync_pending_objects VALUES ('pending', '{"type":"object","id":"pending","data":{"id":"pending","typeId":"legacy","typeVersion":"0.0.0-legacy","title":"Pending","contentJson":{},"propsJson":{},"createdAt":"created","updatedAt":"updated"},"hlc":"pending-hlc"}', 'legacy', 'pending-time');"#,
    )
    .unwrap();
    conn
}

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        params![name],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
        != 0
}

fn version(type_id: &str, version: &str, schema: &str) -> TypeVersion {
    TypeVersion {
        type_id: type_id.into(),
        version: version.into(),
        schema_json: schema.into(),
        ui_schema_json: "{}".into(),
        content_contract_json: "{}".into(),
        relations_json: "[]".into(),
        sync_policy_json: "{}".into(),
        schema_hash: String::new(),
        created_at: "now".into(),
    }
}

#[test]
fn migration_is_lossless_and_repeated_init_is_idempotent() {
    let conn = legacy_db();
    init_schema(&conn).unwrap();
    let snapshot: (i64, i64, i64, String, String) = conn
        .query_row(
            "SELECT (SELECT count(*) FROM objects), (SELECT count(*) FROM object_links),
                    (SELECT count(*) FROM sync_pending_objects),
                    (SELECT updated_at FROM object_types WHERE id='legacy'),
                    (SELECT deleted_at FROM objects WHERE id='deleted')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    let hash: String = conn
        .query_row(
            "SELECT schema_hash FROM object_type_versions WHERE type_id='legacy'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    init_schema(&conn).unwrap();
    assert_eq!(
        snapshot,
        conn.query_row(
            "SELECT (SELECT count(*) FROM objects), (SELECT count(*) FROM object_links),
                (SELECT count(*) FROM sync_pending_objects),
                (SELECT updated_at FROM object_types WHERE id='legacy'),
                (SELECT deleted_at FROM objects WHERE id='deleted')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        )
        .unwrap()
    );
    assert_eq!(
        hash,
        conn.query_row(
            "SELECT schema_hash FROM object_type_versions WHERE type_id='legacy'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap()
    );
    assert_eq!(
        conn.query_row(
            "SELECT type_version FROM objects WHERE id='live'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "0.0.0-legacy"
    );
}

#[test]
fn builtin_startup_does_not_rewrite_existing_definition_or_timestamp() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute(
        "UPDATE object_types SET name='user-owned', schema_json='{\"custom\":true}', updated_at='kept' WHERE id='com.kosmos.note'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE object_type_versions SET schema_json='{\"custom\":true}', schema_hash='kept-hash', created_at='kept-created' WHERE type_id='com.kosmos.note' AND version='1.0.0'",
        [],
    )
    .unwrap();
    init_schema_prerequisites_for_phase3(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT name FROM object_types WHERE id='com.kosmos.note'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "user-owned"
    );
    assert_eq!(
        conn.query_row(
            "SELECT updated_at FROM object_types WHERE id='com.kosmos.note'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "kept"
    );
    assert_eq!(conn.query_row("SELECT schema_hash FROM object_type_versions WHERE type_id='com.kosmos.note' AND version='1.0.0'", [], |r| r.get::<_, String>(0)).unwrap(), "kept-hash");
}

#[test]
fn malformed_schema_or_ui_rolls_back_all_phase2_state() {
    for (schema, ui) in [("", "{}"), ("{", "{}"), ("{}", "[")] {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"PRAGMA foreign_keys=ON; CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER NOT NULL DEFAULT 0); CREATE TABLE objects (id TEXT PRIMARY KEY, type_id TEXT NOT NULL, title TEXT NOT NULL, content_json TEXT NOT NULL, props_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT, FOREIGN KEY(type_id) REFERENCES object_types(id)); INSERT INTO object_types VALUES ('bad','Bad', 'SCHEMA', 'UI', 'c','u',0);"#).unwrap();
        conn.execute(
            "UPDATE object_types SET schema_json=?1, ui_schema_json=?2",
            params![schema, ui],
        )
        .unwrap();
        assert!(init_schema(&conn).is_err());
        assert!(!has_table(&conn, "object_type_versions"));
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(object_types)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!cols.iter().any(|c| c == "current_version"));
    }
}

#[test]
fn canonical_hash_sorts_keys_but_preserves_array_order() {
    let a = json!({"b": {"y": 2, "x": 1}, "a": [1, 2]});
    let b = json!({"a": [1, 2], "b": {"x": 1, "y": 2}});
    let c = json!({"a": [2, 1], "b": {"x": 1, "y": 2}});
    assert_eq!(
        canonical_schema_hash(&a, &json!({}), &json!({}), &json!([]), &json!({})).unwrap(),
        canonical_schema_hash(&b, &json!({}), &json!({}), &json!([]), &json!({})).unwrap()
    );
    assert_ne!(
        canonical_schema_hash(&a, &json!({}), &json!({}), &json!([]), &json!({})).unwrap(),
        canonical_schema_hash(&c, &json!({}), &json!({}), &json!([]), &json!({})).unwrap()
    );
}

#[test]
fn immutable_versions_validate_semver_order_and_conflicts() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(r#"PRAGMA foreign_keys=ON; CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER NOT NULL DEFAULT 0); CREATE TABLE object_type_versions (type_id TEXT NOT NULL, version TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL DEFAULT '{}', content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), FOREIGN KEY(type_id) REFERENCES object_types(id)); INSERT INTO object_types VALUES ('t','T','{}','{}','c','u',0);"#).unwrap();
    for v in ["1.0.0", "1.0.0+build.1", "0.2.0"] {
        insert_type_version(&conn, &version("t", v, "{}"), "now").unwrap();
    }
    assert!(insert_type_version(&conn, &version("t", "1.0", "{}"), "now").is_err());
    assert!(insert_type_version(&conn, &version("t", "0.0.0-legacy", "{}"), "now").is_err());
    assert!(insert_type_version(&conn, &version("t", "1.0.0", "{\"x\":1}"), "now").is_err());
    let versions = list_type_versions(&conn, "t").unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|v| v.version.as_str())
            .collect::<Vec<_>>(),
        vec!["0.2.0", "1.0.0", "1.0.0+build.1"]
    );
}

#[test]
fn aliases_and_pointer_status_base_invariants_are_atomic() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(r#"PRAGMA foreign_keys=ON; CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER NOT NULL DEFAULT 0, owner_kind TEXT NOT NULL DEFAULT 'system', owner_id TEXT, current_version TEXT NOT NULL DEFAULT '0.0.0-legacy', status TEXT NOT NULL DEFAULT 'active', base_type_id TEXT); CREATE TABLE object_type_versions (type_id TEXT NOT NULL, version TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL DEFAULT '{}', content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), FOREIGN KEY(type_id) REFERENCES object_types(id)); CREATE TABLE object_type_aliases (alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT NULL, created_at TEXT NOT NULL, FOREIGN KEY(canonical_type_id) REFERENCES object_types(id)); INSERT INTO object_types VALUES ('a','A','{}','{}','c','u',0,'system',NULL,'1.0.0','active',NULL); INSERT INTO object_types VALUES ('b','B','{}','{}','c','u',0,'system',NULL,'1.0.0','active',NULL);"#).unwrap();
    insert_type_version(&conn, &version("a", "1.0.0", "{}"), "now").unwrap();
    insert_type_version(&conn, &version("b", "1.0.0", "{}"), "now").unwrap();
    register_alias(
        &conn,
        &AliasRecord {
            alias: "old-a".into(),
            canonical_type_id: "a".into(),
            created_at: "now".into(),
        },
    )
    .unwrap();
    register_alias(
        &conn,
        &AliasRecord {
            alias: "old-a".into(),
            canonical_type_id: "a".into(),
            created_at: "later".into(),
        },
    )
    .unwrap();
    assert_eq!(resolve_alias(&conn, "old-a").unwrap().as_deref(), Some("a"));
    assert!(register_alias(
        &conn,
        &AliasRecord {
            alias: "old-a".into(),
            canonical_type_id: "b".into(),
            created_at: "now".into()
        }
    )
    .is_err());
    assert!(register_alias(
        &conn,
        &AliasRecord {
            alias: "a".into(),
            canonical_type_id: "a".into(),
            created_at: "now".into()
        }
    )
    .is_err());
    assert!(set_current_version(&conn, "a", "missing").is_err());
    assert!(set_status(&conn, "a", "invalid").is_err());
    assert!(set_base_type(&conn, "a", Some("a")).is_err());
    set_base_type(&conn, "a", Some("b")).unwrap();
    assert!(set_base_type(&conn, "b", Some("a")).is_err());
    set_current_version(&conn, "a", "1.0.0").unwrap();
}

#[test]
fn registry_mutation_rolls_back_version_alias_and_pointer_together() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(r#"PRAGMA foreign_keys=ON; CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER NOT NULL DEFAULT 0, owner_kind TEXT NOT NULL DEFAULT 'system', owner_id TEXT, current_version TEXT NOT NULL DEFAULT '0.0.0-legacy', status TEXT NOT NULL DEFAULT 'active', base_type_id TEXT); CREATE TABLE object_type_versions (type_id TEXT NOT NULL, version TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL DEFAULT '{}', content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), FOREIGN KEY(type_id) REFERENCES object_types(id)); CREATE TABLE object_type_aliases (alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT NULL, created_at TEXT NOT NULL, FOREIGN KEY(canonical_type_id) REFERENCES object_types(id)); INSERT INTO object_types VALUES ('t','T','{}','{}','c','u',0,'system',NULL,'0.0.0-legacy','active',NULL);"#).unwrap();
    conn.execute_batch("SAVEPOINT registry_test").unwrap();
    insert_type_version(&conn, &version("t", "1.0.0", "{}"), "now").unwrap();
    register_alias(
        &conn,
        &AliasRecord {
            alias: "t-old".into(),
            canonical_type_id: "t".into(),
            created_at: "now".into(),
        },
    )
    .unwrap();
    set_current_version(&conn, "t", "1.0.0").unwrap();
    conn.execute_batch("ROLLBACK TO SAVEPOINT registry_test; RELEASE SAVEPOINT registry_test")
        .unwrap();
    assert!(conn
        .query_row(
            "SELECT 1 FROM object_type_versions WHERE type_id='t' AND version='1.0.0'",
            [],
            |_| Ok(())
        )
        .optional()
        .unwrap()
        .is_none());
    assert!(resolve_alias(&conn, "t-old").unwrap().is_none());
    assert_eq!(
        conn.query_row(
            "SELECT current_version FROM object_types WHERE id='t'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "0.0.0-legacy"
    );
}

#[test]
fn canonical_hash_uses_frozen_top_level_order_and_known_digest() {
    let schema = json!({"b": 2, "a": 1});
    assert_eq!(
        canonical_schema_hash(
            &schema,
            &json!({"ui": true}),
            &json!({}),
            &json!([]),
            &json!({})
        )
        .unwrap(),
        "586b8e06547dc3d15d1db0d0bb0f2ab0ec7499af5f3d1c262dba2ca73f219d52"
    );
}

#[test]
fn registry_contract_shapes_are_rejected_in_migration_and_insert() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER NOT NULL DEFAULT 0); INSERT INTO object_types VALUES ('bad','Bad','[]','{}','c','u',0);").unwrap();
    assert!(init_schema(&conn).is_err());

    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    for (field, value) in [
        ("schema_json", "[]"),
        ("ui_schema_json", "[]"),
        ("content_contract_json", "[]"),
        ("sync_policy_json", "[]"),
        ("relations_json", "{}"),
    ] {
        let mut candidate = version("com.kosmos.note", "1.0.0", "{}");
        match field {
            "schema_json" => candidate.schema_json = value.into(),
            "ui_schema_json" => candidate.ui_schema_json = value.into(),
            "content_contract_json" => candidate.content_contract_json = value.into(),
            "sync_policy_json" => candidate.sync_policy_json = value.into(),
            "relations_json" => candidate.relations_json = value.into(),
            _ => unreachable!(),
        }
        let error = insert_type_version(&conn, &candidate, "caller-time").unwrap_err();
        assert!(error.contains(field), "{field}: {error}");
    }
}

#[test]
fn production_registration_rolls_back_late_alias_failure_and_rejects_alias_collision() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    register_alias(
        &conn,
        &AliasRecord {
            alias: "reserved".into(),
            canonical_type_id: "com.kosmos.note".into(),
            created_at: "now".into(),
        },
    )
    .unwrap();
    let registration = TypeRegistration {
        type_id: "new-type".into(),
        name: "New".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        content_contract_json: "{}".into(),
        relations_json: "[]".into(),
        sync_policy_json: "{}".into(),
        version: "1.0.0".into(),
        schema_hash: String::new(),
        owner_kind: "extension".into(),
        owner_id: Some("owner".into()),
        status: "active".into(),
        base_type_id: Some("com.kosmos.note".into()),
        aliases: vec![AliasRecord {
            alias: "reserved".into(),
            canonical_type_id: "new-type".into(),
            created_at: "now".into(),
        }],
        created_at: "now".into(),
    };
    assert!(register_type(&conn, &registration).is_err());
    for query in [
        "SELECT 1 FROM object_types WHERE id='new-type'",
        "SELECT 1 FROM object_type_versions WHERE type_id='new-type'",
        "SELECT 1 FROM object_type_aliases WHERE canonical_type_id='new-type'",
    ] {
        assert!(
            conn.query_row(query, [], |_| Ok(()))
                .optional()
                .unwrap()
                .is_none(),
            "partial row for {query}"
        );
    }
    let mut successful = registration.clone();
    successful.aliases.clear();
    register_type(&conn, &successful).unwrap();
    let metadata: (String, String, String, String, String) = conn
        .query_row(
            "SELECT owner_kind, owner_id, current_version, status, base_type_id FROM object_types WHERE id='new-type'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        metadata,
        (
            "extension".into(),
            "owner".into(),
            "1.0.0".into(),
            "active".into(),
            "com.kosmos.note".into()
        )
    );
    assert!(register_type(
        &conn,
        &TypeRegistration {
            type_id: "reserved".into(),
            ..registration
        }
    )
    .is_err());
}

#[test]
fn corrupt_persisted_version_listing_fails_closed_without_panic() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES ('com.kosmos.note','not-semver','{}','{}','{}','[]','{}','hash','now')", []).unwrap();
    assert!(list_type_versions(&conn, "com.kosmos.note").is_err());
}

#[test]
fn production_registration_rejects_mismatched_alias_target_before_mutation() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    register_type(
        &conn,
        &TypeRegistration {
            type_id: "other-type".into(),
            name: "Other".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            version: "1.0.0".into(),
            schema_hash: String::new(),
            owner_kind: "extension".into(),
            owner_id: Some("other-owner".into()),
            status: "active".into(),
            base_type_id: None,
            aliases: Vec::new(),
            created_at: "other-time".into(),
        },
    )
    .unwrap();

    let before: (i64, i64, i64, String, String, String) = conn
        .query_row(
            "SELECT
                (SELECT count(*) FROM object_types),
                (SELECT count(*) FROM object_type_versions),
                (SELECT count(*) FROM object_type_aliases),
                (SELECT owner_id FROM object_types WHERE id='other-type'),
                (SELECT current_version FROM object_types WHERE id='other-type'),
                (SELECT status FROM object_types WHERE id='other-type')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();

    let error = register_type(
        &conn,
        &TypeRegistration {
            type_id: "new-type".into(),
            name: "New".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            version: "1.0.0".into(),
            schema_hash: String::new(),
            owner_kind: "extension".into(),
            owner_id: Some("owner".into()),
            status: "active".into(),
            base_type_id: None,
            aliases: vec![AliasRecord {
                alias: "new-alias".into(),
                canonical_type_id: "other-type".into(),
                created_at: "now".into(),
            }],
            created_at: "now".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "alias canonical type mismatch");

    let after: (i64, i64, i64, String, String, String) = conn
        .query_row(
            "SELECT
                (SELECT count(*) FROM object_types),
                (SELECT count(*) FROM object_type_versions),
                (SELECT count(*) FROM object_type_aliases),
                (SELECT owner_id FROM object_types WHERE id='other-type'),
                (SELECT current_version FROM object_types WHERE id='other-type'),
                (SELECT status FROM object_types WHERE id='other-type')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(before, after);
    assert!(conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id='new-type'",
            [],
            |_| Ok(())
        )
        .optional()
        .unwrap()
        .is_none());
}

#[test]
fn production_registration_rejects_existing_canonical_without_any_changes() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    let before: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
    )> = conn
        .prepare("SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked, owner_kind, owner_id, current_version, status, base_type_id FROM object_types ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let before_versions: Vec<(String, String)> = conn
        .prepare("SELECT type_id, version FROM object_type_versions ORDER BY type_id, version")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let before_aliases: Vec<(String, String)> = conn
        .prepare("SELECT alias, canonical_type_id FROM object_type_aliases ORDER BY alias")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    let error = register_type(
        &conn,
        &TypeRegistration {
            type_id: "com.kosmos.note".into(),
            name: "Must Not Replace".into(),
            schema_json: "{\"changed\":true}".into(),
            ui_schema_json: "{\"changed\":true}".into(),
            content_contract_json: "{}".into(),
            relations_json: "[]".into(),
            sync_policy_json: "{}".into(),
            version: "99.0.0".into(),
            schema_hash: String::new(),
            owner_kind: "replacement".into(),
            owner_id: Some("replacement-owner".into()),
            status: "deprecated".into(),
            base_type_id: None,
            aliases: vec![AliasRecord {
                alias: "must-not-exist".into(),
                canonical_type_id: "com.kosmos.note".into(),
                created_at: "later".into(),
            }],
            created_at: "replacement-time".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "canonical type already exists");

    let after: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
    )> = conn
        .prepare("SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked, owner_kind, owner_id, current_version, status, base_type_id FROM object_types ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let after_versions: Vec<(String, String)> = conn
        .prepare("SELECT type_id, version FROM object_type_versions ORDER BY type_id, version")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let after_aliases: Vec<(String, String)> = conn
        .prepare("SELECT alias, canonical_type_id FROM object_type_aliases ORDER BY alias")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(before_versions, after_versions);
    assert_eq!(before_aliases, after_aliases);
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM object_type_aliases WHERE alias='must-not-exist'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
