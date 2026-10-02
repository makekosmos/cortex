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

    conn.execute_batch(concat!(
        "CREATE INDEX idx_sync_pending_awaited_type_version ON sync_pending_objects(",
        "awaited_type_id, awaited_type_version)"
    ))
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
                .execute_batch(concat!(
                    "CREATE TABLE pending_ref(id TEXT REFERENCES sync_pending_objects(id)); ",
                    "INSERT INTO pending_ref VALUES ('o1')"
                ))
                .unwrap(),
            "index" => conn
                .execute_batch(concat!(
                    "DROP INDEX idx_sync_pending_awaited_type_version; CREATE TABLE ",
                    "index_name_owner(value TEXT); CREATE INDEX ",
                    "idx_sync_pending_awaited_type_version ON index_name_owner(value)"
                ))
                .unwrap(),
            other => panic!("unknown failure stage {other}"),
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
    conn.execute(
        concat!(
            "INSERT INTO canonical_migration_items(contract_version,source_kind,",
            "source_id,source_hash,status,updated_at) VALUES ('phase3-canonical-v1',",
            "'test','x','h','unchanged','now')"
        ),
        [],
    )
    .unwrap();
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
