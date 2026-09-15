// KOS-51: unit-level snapshot restore тесты. RPC-level покрытие —
// src/main/tests/snapshot_restore.rs.

    /// File-backed DB в tempdir (restore работает с `db_path`, in-memory
    /// не подходит). Возвращает (TempDir, db_path, conn).
    fn setup_file_db() -> (tempfile::TempDir, String, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        let db_str = db_path.to_str().unwrap().to_string();
        let conn = open_db(&db_str).unwrap();
        init_schema(&conn).unwrap();
        (dir, db_str, conn)
    }

    fn seed_object(conn: &Connection, id: &str, title: &str) {
        upsert_object_type(conn, &make_object_type("snapshot_type", "Note")).unwrap();
        upsert_object(conn, &make_object(id, "snapshot_type", title)).unwrap();
    }

    /// Пишет snapshot текущей live DB в `<db_dir>/backups/<name>`.
    fn make_snapshot(conn: &Connection, db_path: &str, name: &str) {
        let dir = backups_dir(db_path);
        fs::create_dir_all(&dir).unwrap();
        backup_to_file(conn, dir.join(name).to_str().unwrap()).unwrap();
    }

    #[test]
    fn snapshot_id_validation_accepts_only_plain_basenames() {
        assert!(validate_snapshot_id("ark.db.backup-2026-09-15").is_ok());
        for bad in [
            "",
            ".hidden",
            "..",
            "../ark.db",
            "..\\ark.db",
            "sub/dir.db",
            "sub\\dir.db",
            "C:\\abs.db",
            "/abs.db",
        ] {
            assert!(validate_snapshot_id(bad).is_err(), "rejected: {bad:?}");
        }
    }

    #[test]
    fn list_snapshots_returns_only_regular_files() {
        let (dir, db_path, conn) = setup_file_db();
        seed_object(&conn, "obj-1", "one");
        make_snapshot(&conn, &db_path, "ark.db.backup-a");
        // Noise: dot-staging file, nested dir, broken subpath — не попадают в list.
        let backups = backups_dir(&db_path);
        fs::write(backups.join(".restore-src-tmp.db"), b"junk").unwrap();
        fs::create_dir(backups.join("nested")).unwrap();

        let entries = list_snapshots(&db_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "ark.db.backup-a");
        assert!(entries[0].size_bytes > 0);
        drop(conn);
        drop(dir);
    }

    /// AC5: провал post-restore verification откатывает live DB в
    /// pre-restore состояние — НЕ в состояние snapshot'а и не в кашу.
    #[test]
    fn restore_rolls_back_to_pre_restore_state_when_post_verify_fails() {
        let (_dir, db_path, mut conn) = setup_file_db();
        seed_object(&conn, "obj-snap", "in snapshot");
        make_snapshot(&conn, &db_path, "ark.db.backup-rollback");

        // Live DB расходится со snapshot'ом: объект удалён ПОСЛЕ backup'а.
        conn.execute("DELETE FROM objects WHERE id = 'obj-snap'", [])
            .unwrap();
        assert!(list_objects(&conn).unwrap().is_empty());

        let failing_verify = |_conn: &Connection, _fp: &[String]| -> Result<(), String> {
            Err("injected post-verify failure".to_string())
        };
        let result =
            restore_snapshot_impl(&mut conn, &db_path, "ark.db.backup-rollback", &failing_verify);
        let err = result.expect_err("post-verify failure must fail the restore");
        assert!(
            err.contains("rolled back"),
            "error must report rollback: {err}"
        );

        // Rollback = pre-restore состояние (объект удалён), не snapshot.
        assert!(
            list_objects(&conn).unwrap().is_empty(),
            "rollback must restore the PRE-RESTORE live state, not the snapshot"
        );
        check_integrity(&conn).unwrap();
    }

    #[test]
    fn restore_rejects_traversal_and_missing_ids_without_touching_live() {
        let (_dir, db_path, mut conn) = setup_file_db();
        seed_object(&conn, "obj-live", "alive");
        make_snapshot(&conn, &db_path, "ark.db.backup-ok");

        for bad in ["../ark.db", "sub/dir.db", "missing.db"] {
            let result = restore_snapshot(&mut conn, &db_path, bad);
            assert!(result.is_err(), "must reject {bad:?}");
        }
        // Traversal на живой файл тоже не должен его трогать.
        let objects = list_objects(&conn).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "obj-live");
    }

    #[test]
    fn validate_reports_integrity_and_schema_verdicts() {
        let (_dir, db_path, conn) = setup_file_db();
        seed_object(&conn, "obj-v", "v");
        make_snapshot(&conn, &db_path, "ark.db.backup-valid");

        let ok = validate_snapshot(&conn, &db_path, "ark.db.backup-valid").unwrap();
        assert!(ok.exists && ok.integrity_ok && ok.schema_match && ok.valid);

        // Не-SQLite garbage.
        fs::write(backups_dir(&db_path).join("ark.db.backup-garbage"), b"not a sqlite db").unwrap();
        let garbage = validate_snapshot(&conn, &db_path, "ark.db.backup-garbage").unwrap();
        assert!(garbage.exists);
        assert!(!garbage.integrity_ok);
        assert!(!garbage.valid);

        // Валидный SQLite, но чужая schema.
        let foreign_path = backups_dir(&db_path).join("ark.db.backup-foreign");
        let foreign = Connection::open(&foreign_path).unwrap();
        foreign
            .execute_batch("CREATE TABLE other(id TEXT PRIMARY KEY)")
            .unwrap();
        drop(foreign);
        let mismatched = validate_snapshot(&conn, &db_path, "ark.db.backup-foreign").unwrap();
        assert!(mismatched.integrity_ok);
        assert!(!mismatched.schema_match);
        assert!(!mismatched.valid);

        let missing = validate_snapshot(&conn, &db_path, "ark.db.backup-nope").unwrap();
        assert!(!missing.exists && !missing.valid);
    }

    #[test]
    fn restore_replaces_live_content_and_stays_valid() {
        let (_dir, db_path, mut conn) = setup_file_db();
        seed_object(&conn, "obj-a", "before");
        make_snapshot(&conn, &db_path, "ark.db.backup-1");

        // Расходимся: удаляем snapshot-объект, добавляем новый.
        conn.execute("DELETE FROM objects WHERE id = 'obj-a'", [])
            .unwrap();
        upsert_object(&conn, &make_object("obj-b", "snapshot_type", "after")).unwrap();

        let report = restore_snapshot(&mut conn, &db_path, "ark.db.backup-1").unwrap();
        assert!(report.restored);
        assert_eq!(report.objects, 1);

        let objects = list_objects(&conn).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "obj-a");
        check_integrity(&conn).unwrap();
    }
