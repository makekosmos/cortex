
#[test]

fn registry_mutation_rolls_back_version_alias_and_pointer_together() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(concat!(r#"PRAGMA foreign_keys=ON; CREATE TABLE object_types (id TEXT PRIMARY KEY, "#,r#"name TEXT NOT NULL, schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL,"#,r#" created_at TEXT NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER "#,r#"NOT NULL DEFAULT 0, owner_kind TEXT NOT NULL DEFAULT 'system', owner_id "#,r#"TEXT, current_version TEXT NOT NULL DEFAULT '0.0.0-legacy', status TEXT NOT "#,r#"NULL DEFAULT 'active', base_type_id TEXT); CREATE TABLE "#,r#"object_type_versions (type_id TEXT NOT NULL, version TEXT NOT NULL, "#,r#"schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL DEFAULT '{}', "#,r#"content_contract_json TEXT NOT NULL DEFAULT '{}', relations_json TEXT NOT "#,r#"NULL DEFAULT '[]', sync_policy_json TEXT NOT NULL DEFAULT '{}', schema_hash "#,r#"TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(type_id,version), "#,r#"FOREIGN KEY(type_id) REFERENCES object_types(id)); CREATE TABLE "#,r#"object_type_aliases (alias TEXT PRIMARY KEY, canonical_type_id TEXT NOT "#,r#"NULL, created_at TEXT NOT NULL, FOREIGN KEY(canonical_type_id) REFERENCES "#,r#"object_types(id)); INSERT INTO object_types VALUES ('t','T','{}','{}','c',"#,r#"'u',0,'system',NULL,'0.0.0-legacy','active',NULL);"#)).unwrap();
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
    conn.execute_batch(concat!("CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, ","schema_json TEXT NOT NULL, ui_schema_json TEXT NOT NULL, created_at TEXT ","NOT NULL, updated_at TEXT NOT NULL, system_locked INTEGER NOT NULL DEFAULT ","0); INSERT INTO object_types VALUES ('bad','Bad','[]','{}','c','u',0);")).unwrap();
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
            other => panic!("unknown field {other}"),
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
            concat!("SELECT owner_kind, owner_id, current_version, status, base_type_id FROM ","object_types WHERE id='new-type'"),
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
    conn.execute(concat!("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,","content_contract_json,relations_json,sync_policy_json,schema_hash,","created_at) VALUES ('com.kosmos.note','not-semver','{}','{}','{}','[]','{}',","'hash','now')"), []).unwrap();
    assert!(list_type_versions(&conn, "com.kosmos.note").is_err());
}