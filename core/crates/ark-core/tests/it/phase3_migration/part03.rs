
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

