// KOS-51: RPC-level покрытие db_backup_list/validate/restore.

use super::*;

async fn init_snapshot_db() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("ark.db");
    let db_str = db_path.to_string_lossy().to_string();
    handle_request(Request::Init {
        db_path: db_str.clone(),
    })
    .await
    .unwrap();
    (dir, db_str)
}

async fn seed_rpc_object(id: &str, title: &str) {
    handle_request(Request::UpsertObjectType {
        object_type: ObjectType {
            id: "snap_type".to_string(),
            name: "Note".to_string(),
            schema_json: "{}".to_string(),
            ui_schema_json: "{}".to_string(),
            created_at: "2026-09-15T00:00:00.000Z".to_string(),
            updated_at: "2026-09-15T00:00:00.000Z".to_string(),
            system_locked: false,
        },
        device_id: Some("test-device".to_string()),
    })
    .await
    .unwrap();
    handle_request(Request::UpsertObject {
        object: ArkObjectWrite {
            id: id.to_string(),
            type_id: "snap_type".to_string(),
            type_version: Some("0.0.0-legacy".to_string()),
            title: title.to_string(),
            content_json: json!({}),
            props_json: json!({}),
            created_at: "2026-09-15T00:00:00.000Z".to_string(),
            updated_at: "2026-09-15T00:00:00.000Z".to_string(),
            deleted_at: None,
        },
        expected_snapshot: None,
        device_id: Some("test-device".to_string()),
    })
    .await
    .unwrap();
}

/// Пишет snapshot live DB в Core-owned `backups/` через Online Backup.
fn write_snapshot(db_path: &str, id: &str) {
    let dir = db::backups_dir(db_path);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join(id);
    with_conn(|conn| db::backup_to_file(conn, dest.to_str().unwrap())).unwrap();
}

fn live_object_ids() -> Vec<String> {
    with_conn(|conn| {
        let objects = db::list_objects(conn)?;
        Ok(objects.into_iter().map(|o| o.id).collect::<Vec<_>>())
    })
    .unwrap()
}

/// AC1+AC4: restore через RPC заменяет live содержимое и переживает
/// restart-equivalent (re-Init → reopen того же файла).
#[tokio::test]
async fn db_backup_restore_roundtrip_survives_reinit() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let (_dir, db_path) = init_snapshot_db().await;
    seed_rpc_object("obj-before", "before").await;
    write_snapshot(&db_path, "ark.db.backup-rt");

    // Live расходится со snapshot'ом.
    handle_request(Request::DeleteObject {
        id: "obj-before".to_string(),
        expected_snapshot: None,
        device_id: Some("test-device".to_string()),
    })
    .await
    .unwrap();
    seed_rpc_object("obj-after", "after").await;
    assert_eq!(live_object_ids(), vec!["obj-after".to_string()]);

    let report = handle_request(Request::DbBackupRestore {
        backup_id: "ark.db.backup-rt".to_string(),
    })
    .await
    .unwrap();
    assert_eq!(report["restored"], json!(true));
    assert_eq!(report["id"], json!("ark.db.backup-rt"));
    assert_eq!(report["objects"], json!(1));

    assert_eq!(live_object_ids(), vec!["obj-before".to_string()]);

    // Restart-equivalent: re-Init переоткрывает файл с диска.
    handle_request(Request::Init {
        db_path: db_path.clone(),
    })
    .await
    .unwrap();
    assert_eq!(live_object_ids(), vec!["obj-before".to_string()]);
}

/// AC3: невалидные id, отсутствующие, garbage и чужие схемы не трогают live DB.
#[tokio::test]
async fn db_backup_restore_rejects_invalid_sources_without_touching_live() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let (_dir, db_path) = init_snapshot_db().await;
    seed_rpc_object("obj-live", "alive").await;
    write_snapshot(&db_path, "ark.db.backup-good");

    // Не-SQLite garbage.
    std::fs::write(
        db::backups_dir(&db_path).join("ark.db.backup-garbage"),
        b"not a sqlite database",
    )
    .unwrap();
    // Валидный SQLite с чужой schema.
    let foreign_path = db::backups_dir(&db_path).join("ark.db.backup-foreign");
    let foreign = rusqlite::Connection::open(&foreign_path).unwrap();
    foreign
        .execute_batch("CREATE TABLE other(id TEXT PRIMARY KEY)")
        .unwrap();
    drop(foreign);

    for bad_id in [
        "../ark.db",
        "..\\ark.db",
        "sub/dir.db",
        ".restore-src-x.db",
        "ark.db.backup-missing",
        "ark.db.backup-garbage",
        "ark.db.backup-foreign",
    ] {
        let result = handle_request(Request::DbBackupRestore {
            backup_id: bad_id.to_string(),
        })
        .await;
        assert!(result.is_err(), "must reject {bad_id:?}");
    }

    assert_eq!(live_object_ids(), vec!["obj-live".to_string()]);
    with_conn(db::check_integrity).unwrap();
}

/// AC6: list возвращает только regular files из backups/; validate даёт
/// per-check typed verdict.
#[tokio::test]
async fn db_backup_list_and_validate_contract() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let (_dir, db_path) = init_snapshot_db().await;
    seed_rpc_object("obj-l", "l").await;
    write_snapshot(&db_path, "ark.db.backup-listed");

    let backups = db::backups_dir(&db_path);
    std::fs::write(backups.join(".staging-tmp"), b"junk").unwrap();
    std::fs::create_dir(backups.join("nested-dir")).unwrap();

    let listed = handle_request(Request::DbBackupList).await.unwrap();
    let ids = listed["backups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["ark.db.backup-listed".to_string()]);
    assert!(listed["backups"][0]["size_bytes"].as_u64().unwrap() > 0);

    let ok = handle_request(Request::DbBackupValidate {
        backup_id: "ark.db.backup-listed".to_string(),
    })
    .await
    .unwrap();
    assert_eq!(ok["valid"], json!(true));
    assert_eq!(ok["integrity_ok"], json!(true));
    assert_eq!(ok["schema_match"], json!(true));

    let missing = handle_request(Request::DbBackupValidate {
        backup_id: "ark.db.backup-nope".to_string(),
    })
    .await
    .unwrap();
    assert_eq!(missing["exists"], json!(false));
    assert_eq!(missing["valid"], json!(false));

    let traversal = handle_request(Request::DbBackupValidate {
        backup_id: "../ark.db".to_string(),
    })
    .await
    .unwrap();
    assert_eq!(traversal["valid"], json!(false));
}

/// AC2: restore ждёт BACKUP_GATE, пока активный backup не завершится.
/// Gate удерживается в этом потоке; restore на отдельном потоке обязан
/// завершиться только после release.
#[test]
fn db_backup_restore_waits_for_backup_gate() {
    let _guard = TEST_DB_MUTEX.blocking_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (_dir, db_path) = runtime.block_on(async {
        let pair = init_snapshot_db().await;
        seed_rpc_object("obj-g", "g").await;
        pair
    });
    write_snapshot(&db_path, "ark.db.backup-gated");

    let gate = BACKUP_GATE.lock().unwrap_or_else(|e| e.into_inner());
    let order = Arc::new(StdMutex::new(Vec::<&'static str>::new()));
    let order_in_thread = order.clone();
    let restore_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = rt.block_on(handle_request(Request::DbBackupRestore {
            backup_id: "ark.db.backup-gated".to_string(),
        }));
        order_in_thread
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push("restore");
        result
    });

    std::thread::sleep(std::time::Duration::from_millis(300));
    order
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push("gate_released");
    drop(gate);

    let result = restore_thread.join().unwrap();
    assert!(result.is_ok(), "restore must succeed after gate release");
    assert_eq!(
        *order.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["gate_released", "restore"],
        "restore must not run while BACKUP_GATE is held"
    );
}

#[test]
fn db_backup_snapshot_ops_parse_snake_case_wire_format() {
        for (operation, extra) in [
            ("db_backup_list", json!({})),
            ("db_backup_validate", json!({"backup_id": "x"})),
            ("db_backup_restore", json!({"backup_id": "x"})),
        ] {
            let mut value = json!({"operation": operation});
            value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            assert!(
                serde_json::from_value::<Request>(value).is_ok(),
                "{operation} must parse"
            );
        }
        // camelCase variants не принимаются.
        assert!(serde_json::from_value::<Request>(json!({"operation": "dbBackupRestore"})).is_err());
}
