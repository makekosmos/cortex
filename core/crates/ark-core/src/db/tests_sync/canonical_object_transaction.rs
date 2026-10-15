#[test]
fn failed_outer_apply_commit_rolls_back_and_releases_connection() {
    let conn = setup_db();
    conn.execute_batch(
        "CREATE TABLE deferred_probe (
             parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TRIGGER add_deferred_probe AFTER INSERT ON objects
         BEGIN
             INSERT INTO deferred_probe(parent_id) VALUES ('missing-parent');
         END;",
    )
    .unwrap();

    let entity = sync_object("deferred-commit-failure", "note_obj", "Deferred failure");
    let error = SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap_err();
    assert!(!error.is_empty());
    assert!(
        conn.is_autocommit(),
        "failed outer RELEASE must roll back the transaction"
    );
    assert!(get_object(&conn, &entity.id).unwrap().is_none());

    conn.execute_batch("DROP TRIGGER add_deferred_probe")
        .unwrap();
    let next = sync_object("after-deferred-failure", "note_obj", "Next write");
    SqliteStorageBackend::apply_entity_blocking(&conn, &next).unwrap();
    assert!(get_object(&conn, &next.id).unwrap().is_some());
}

#[tokio::test]
async fn failed_authenticated_core_task_commit_rolls_back_every_write_and_releases_connection() {
    let conn = setup_db();
    conn.execute_batch(
        "CREATE TABLE deferred_core_task_probe (
             parent_id TEXT REFERENCES objects(id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TRIGGER fail_core_task_commit AFTER INSERT ON objects
         BEGIN
             INSERT INTO deferred_core_task_probe(parent_id) VALUES ('missing-parent');
         END;",
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(conn));
    let backend = SqliteStorageBackend::new(shared.clone());
    let entity = full_core_task_entity(
        "failed-core-task-commit",
        "Commit must roll back",
        "2026-10-15T00:00:00.000Z:000001:device-a",
    );

    let error = backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap_err();
    assert!(!error.is_empty());
    {
        let conn = shared.lock().unwrap();
        assert!(
            conn.is_autocommit(),
            "failed COMMIT must release the connection"
        );
        assert!(get_object(&conn, &entity.id).unwrap().is_none());
        let fts_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM object_search_fts WHERE object_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_rows, 0);
        let sync_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sync_rows, 0);
        let receipt_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipt_rows, 0);
        assert!(!get_sync_kv(&conn, "lan_sync.version_vector")
            .unwrap_or_default()
            .unwrap_or_default()
            .contains(&entity.id));
        conn.execute_batch("DROP TRIGGER fail_core_task_commit")
            .unwrap();
    }

    let next = full_core_task_entity(
        "after-core-task-commit-failure",
        "Connection remains usable",
        "2026-10-15T00:00:00.000Z:000002:device-a",
    );
    backend
        .apply_authenticated_entity(&next, "device-a")
        .await
        .unwrap();
    let conn = shared.lock().unwrap();
    assert!(get_object(&conn, &next.id).unwrap().is_some());
}

#[tokio::test]
async fn authenticated_apply_rechecks_removed_peer_before_persisting() {
    let conn = setup_db();
    set_sync_kv(&conn, "sync.removed_peers", r#"["device-blocked"]"#).unwrap();
    let backend = SqliteStorageBackend::new(Arc::new(Mutex::new(conn)));
    let mut entity = sync_object("blocked-core-task", "core.task", "Blocked task");
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );

    let result = backend
        .apply_authenticated_entity(&entity, "device-blocked")
        .await;
    assert!(
        result.is_err(),
        "blocked authenticated peer must be rejected"
    );
    let conn = backend.conn.lock().unwrap();
    assert!(get_object(&conn, &entity.id).unwrap().is_none());
}

fn full_core_task_entity(id: &str, title: &str, hlc: &str) -> SyncEntity {
    let mut entity = sync_object(id, "core.task", title);
    entity.hlc = hlc.into();
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );
    entity
}

#[tokio::test]
async fn clear_all_removes_object_write_receipts_before_replay() {
    let conn = setup_db();
    let backend = SqliteStorageBackend::new(Arc::new(Mutex::new(conn)));
    let mut entity = sync_object("clear-receipt-task", "core.task", "Task before reset");
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );
    backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap();
    {
        let conn = backend.conn.lock().unwrap();
        assert!(get_object(&conn, &entity.id).unwrap().is_some());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        clear_all(&conn).unwrap();
        assert!(get_object(&conn, &entity.id).unwrap().is_none());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
    entity.hlc = "2026-01-02T00:00:00.000Z:000001:device-a".into();
    backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap();
    let conn = backend.conn.lock().unwrap();
    assert!(get_object(&conn, &entity.id).unwrap().is_some());
}

#[tokio::test]
async fn authenticated_core_task_replay_is_idempotent_and_rejects_revision_reuse() {
    let backend = SqliteStorageBackend::new(Arc::new(Mutex::new(setup_db())));
    let mut entity = sync_object("receipt-replay-task", "core.task", "Original title");
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );
    backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap();
    backend
        .apply_authenticated_entity(&entity, "relay-peer")
        .await
        .unwrap();
    {
        let conn = backend.conn.lock().unwrap();
        let receipt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&entity.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipt_count, 1, "replay must reuse its committed receipt");
        assert_eq!(
            get_object(&conn, &entity.id).unwrap().unwrap().title,
            "Original title"
        );
    }

    let mut stale = entity.clone();
    stale.hlc = "2025-12-31T00:00:00.000Z:000001:device-a".into();
    stale
        .data
        .insert("propsJson".into(), json!("malformed stale payload"));
    backend
        .apply_authenticated_entity(&stale, "device-a")
        .await
        .unwrap();
    {
        let conn = backend.conn.lock().unwrap();
        assert_eq!(
            get_object(&conn, &entity.id).unwrap().unwrap().title,
            "Original title"
        );
        let stale_receipts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts
                 WHERE entity_id = ?1 AND source_hlc = ?2",
                params![entity.id, stale.hlc],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale_receipts, 0, "legacy stale no-op writes no receipt");
    }

    entity.data.insert("title".into(), json!("Changed payload"));
    assert_eq!(
        backend
            .apply_authenticated_entity(&entity, "device-a")
            .await
            .unwrap_err(),
        "object_revision_payload_mismatch"
    );
    let conn = backend.conn.lock().unwrap();
    assert_eq!(
        get_object(&conn, &entity.id).unwrap().unwrap().title,
        "Original title"
    );
}
