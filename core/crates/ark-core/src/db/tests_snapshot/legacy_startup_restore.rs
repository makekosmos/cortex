use super::*;

/// KOS-428: old-format databases are migrated on startup before backups are
/// made. The resulting current-schema backup remains verifiable and restorable.
#[test]
fn startup_migrates_legacy_database_before_verified_backup_and_restore() {
    let dir = crate::test_support::temp_dir();
    let db_path = dir.path().join("ark.db");
    let db_str = db_path.to_str().unwrap().to_string();

    // This is the pre-ARK legacy task schema: startup must add the current
    // tables and migrate the row before snapshotting it.
    let legacy = Connection::open(&db_path).unwrap();
    legacy
        .execute_batch(
            "CREATE TABLE todos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                notes TEXT,
                priority INTEGER NOT NULL DEFAULT 0,
                scheduled_date TEXT,
                deadline TEXT,
                reminder_date TEXT,
                is_today INTEGER NOT NULL DEFAULT 0,
                is_evening INTEGER NOT NULL DEFAULT 0,
                is_someday INTEGER NOT NULL DEFAULT 0,
                is_completed INTEGER NOT NULL DEFAULT 0,
                completed_at TEXT,
                is_cancelled INTEGER NOT NULL DEFAULT 0,
                cancelled_at TEXT,
                is_trashed INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                heading_id TEXT,
                project_id TEXT,
                area_id TEXT,
                tag_ids TEXT NOT NULL DEFAULT '[]',
                checklist_items TEXT NOT NULL DEFAULT '[]',
                recurrence_rule TEXT,
                created_at TEXT NOT NULL
            );
            INSERT INTO todos(id, title, notes, priority, created_at)
                VALUES ('legacy-task', 'Keep through restore', 'legacy notes', 2,
                        '2026-10-19T00:00:00.000Z');",
        )
        .unwrap();
    drop(legacy);

    let mut conn = ark_store::open::open_db(&db_str).unwrap();
    init_schema(&conn).unwrap();
    let migrated = ark_store::load_all::load_all(&conn).unwrap();
    assert_eq!(migrated.todos.len(), 1);
    assert_eq!(migrated.todos[0].id, "legacy-task");

    let snapshot_id = "ark.db.backup-v1-legacy-migration";
    fs::create_dir_all(backups_dir(&db_str)).unwrap();
    let snapshot_path = backups_dir(&db_str).join(snapshot_id);
    ark_store::snapshot::backup_to_file_chunked_verified(
        &db_str,
        snapshot_path.to_str().unwrap(),
        4,
        std::time::Duration::ZERO,
    )
    .unwrap();
    let validation = validate_snapshot(&conn, &db_str, snapshot_id).unwrap();
    assert!(
        validation.valid,
        "migrated snapshot should validate: {validation:?}"
    );

    conn.execute(
        "UPDATE todos SET title = 'live change' WHERE id = 'legacy-task'",
        [],
    )
    .unwrap();
    assert_eq!(
        ark_store::load_all::load_all(&conn).unwrap().todos[0].title,
        "live change"
    );

    let report = restore_snapshot(&mut conn, &db_str, snapshot_id).unwrap();
    assert!(report.restored);
    assert_eq!(report.objects, 1);
    let restored = ark_store::load_all::load_all(&conn).unwrap();
    assert_eq!(restored.todos.len(), 1);
    assert_eq!(restored.todos[0].id, "legacy-task");
    assert_eq!(restored.todos[0].title, "Keep through restore");
    ark_store::open::check_integrity(&conn).unwrap();
}
