#[test]
fn object_fault_rolls_back_every_projection_and_preserves_preexisting_rows() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at) VALUES('sentinel','note_obj','0.0.0-legacy','s',",
            "'{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}','{}','c','u')"
        ),
        [],
    )
    .unwrap();
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM objects", [], |r| r.get(0))
        .unwrap();
    assert!(migrate_phase3_with_options(
        &conn,
        &MigrationOptions {
            fail_after_objects: Some(1),
            ..Default::default()
        }
    )
    .is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM objects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        before
    );
    for table in [
        "object_local_state",
        "object_sync_versions",
        "object_migration_quarantine",
        "canonical_migration_runs",
        "canonical_migration_items",
    ] {
        assert!(
            !table_exists(&conn, table)
                || conn
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                    .unwrap()
                    == 0
        );
    }
}

#[test]
fn completed_rerun_is_exact_noop_and_source_or_canonical_changes_conflict() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    let first = migrate_phase3(&conn).unwrap();
    let snapshot: (i64, String, Option<String>, i64) = conn
        .query_row(
            concat!(
                "SELECT (SELECT COUNT(*) FROM canonical_migration_items), (SELECT started_at ",
                "FROM canonical_migration_runs), (SELECT completed_at FROM ",
                "canonical_migration_runs), (SELECT COALESCE(SUM(attempt),0) FROM ",
                "canonical_migration_items)"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    let second = migrate_phase3(&conn).unwrap();
    assert_eq!(first.source_inventory_hash, second.source_inventory_hash);
    assert_eq!(second.unchanged, 1);
    assert_eq!(
        snapshot,
        conn.query_row(
            "SELECT (SELECT COUNT(*) FROM canonical_migration_items), (SELECT started_at \
FROM canonical_migration_runs), (SELECT completed_at FROM \
canonical_migration_runs), (SELECT COALESCE(SUM(attempt),0) FROM \
canonical_migration_items)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        )
        .unwrap()
    );
    conn.execute("UPDATE objects SET props_json='{}' WHERE id='n'", [])
        .unwrap();
    assert!(migrate_phase3(&conn).is_err());
}

#[test]
fn changed_source_conflicts_without_mutating_the_completed_migration() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    conn.execute(
        concat!(
            "INSERT INTO todos(id,title,notes,priority,project_id,tag_ids,",
            "checklist_items,created_at) VALUES('source-todo','Original','n',2,NULL,'[]',",
            "'[]','2026-01-01T00:00:00Z')"
        ),
        [],
    )
    .unwrap();
    migrate_phase3(&conn).unwrap();
    let before: (String, i64, i64) = conn
        .query_row(
            concat!(
                "SELECT (SELECT notes FROM todos WHERE id='source-todo'),(SELECT COUNT(*) ",
                "FROM objects),(SELECT COUNT(*) FROM canonical_migration_items)"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    conn.execute(
        "UPDATE todos SET title='Changed at source' WHERE id='source-todo'",
        [],
    )
    .unwrap();
    assert!(migrate_phase3(&conn).is_err());
    assert_eq!(
        before,
        conn.query_row(
            concat!(
                "SELECT (SELECT notes FROM todos WHERE id='source-todo'),(SELECT COUNT(*) ",
                "FROM objects),(SELECT COUNT(*) FROM canonical_migration_items)"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    );
}

#[test]
fn archive_schema_collision_blocks_before_any_object_mutation() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    conn.execute(
        concat!(
            "CREATE TABLE legacy_type_definition_archive(type_id TEXT PRIMARY KEY,",
            "summary_json TEXT NOT NULL,versions_json TEXT NOT NULL,inbound_aliases_json ",
            "TEXT NOT NULL,source_hash TEXT NOT NULL,archived_at TEXT NOT NULL)"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO legacy_type_definition_archive VALUES('note_obj','wrong',",
            "'wrong','wrong','wrong','old')"
        ),
        [],
    )
    .unwrap();
    let before: String = conn
        .query_row(
            "SELECT summary_json||source_hash FROM legacy_type_definition_archive",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(migrate_phase3(&conn).is_err());
    assert_eq!(
        before,
        conn.query_row(
            "SELECT summary_json||source_hash FROM legacy_type_definition_archive",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap()
    );
    assert_eq!(
        conn.query_row("SELECT type_version FROM objects WHERE id='n'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "0.0.0-legacy"
    );
}

#[test]
fn orchestrator_has_no_publication_or_sync_side_effects() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    migrate_phase3(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM object_sync_versions", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        !table_exists(&conn, "object_search_fts")
            || conn
                .query_row("SELECT COUNT(*) FROM object_search_fts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap()
                == 0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM canonical_migration_runs", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        1
    );
}

#[test]
fn alias_target_collision_blocks_before_any_object_mutation() {
    let conn = db();
    legacy(&conn, "n", "note_obj", json!({"description":"x"}));
    legacy(&conn, "task", "task_obj", json!({}));
    conn.execute(
        concat!(
            "INSERT INTO object_type_aliases(alias,canonical_type_id,created_at) VALUES(",
            "'note_obj','task_obj','old')"
        ),
        [],
    )
    .unwrap();
    assert!(migrate_phase3(&conn).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT type_id,type_version FROM objects WHERE id='n'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        ("note_obj".into(), "0.0.0-legacy".into())
    );
    assert!(!table_exists(&conn, "canonical_migration_runs"));
}

#[test]
fn durable_resume_reopens_committed_partial_ledger_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("phase3-resume.sqlite");
    {
        let conn = Connection::open(&path).unwrap();
        init_schema_prerequisites_for_phase3(&conn).unwrap();
        legacy(&conn, "n", "note_obj", json!({"description":"x"}));
        conn.execute(
            concat!(
                "INSERT INTO todos(id,title,notes,priority,project_id,tag_ids,",
                "checklist_items,created_at) VALUES('resume-source','Resume me','n',2,NULL,",
                "'[]','[]','2026-01-01T00:00:00Z')"
            ),
            [],
        )
        .unwrap();
        migrate_phase3(&conn).unwrap();
        let source_id: String = conn
            .query_row(
                concat!(
                    "SELECT source_id FROM canonical_migration_items WHERE source_kind LIKE ",
                    "'native:%' ORDER BY source_id LIMIT 1"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(source_id, "resume-source");
        conn.execute("DELETE FROM objects WHERE id=?1", [&source_id])
            .unwrap();
        conn.execute(
            concat!(
                "UPDATE canonical_migration_items SET status='pending',checkpoint='prepared',",
                "attempt=0 WHERE source_kind LIKE 'native:%' AND source_id=?1"
            ),
            [&source_id],
        )
        .unwrap();
        conn.execute(
            concat!(
                "UPDATE canonical_migration_runs SET status='running',completed_at=NULL ",
                "WHERE contract_version='phase3-canonical-v1'"
            ),
            [],
        )
        .unwrap();
    }
    let reopened = Connection::open(&path).unwrap();
    let report = migrate_phase3(&reopened).unwrap();
    assert_eq!(report.status, "completed");
    assert_eq!(
        reopened
            .query_row(
                "SELECT status FROM canonical_migration_items WHERE source_kind LIKE \
'native:%' AND source_id='resume-source'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "migrated"
    );
    assert_eq!(
        reopened
            .query_row(
                "SELECT attempt FROM canonical_migration_items WHERE source_kind LIKE \
'native:%' AND source_id='resume-source'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id='resume-source'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}
