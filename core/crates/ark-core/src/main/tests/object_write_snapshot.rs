use super::*;

async fn snapshot(id: &str) -> ObjectWriteSnapshot {
    serde_json::from_value(
        handle_request(Request::GetObjectWriteSnapshot { id: id.to_string() })
            .await
            .unwrap(),
    )
    .unwrap()
}

fn task(id: &str, title: &str) -> ArkObjectWrite {
    let mut object = canonical_task_object(Some("1.0.0"), canonical_task_props());
    object.id = id.to_string();
    object.title = title.to_string();
    object
}

async fn guarded_upsert(
    object: ArkObjectWrite,
    expected_snapshot: ObjectWriteSnapshot,
) -> Result<Value, String> {
    handle_request(Request::UpsertObject {
        object,
        expected_snapshot: Some(expected_snapshot),
        device_id: Some("snapshot-test".to_string()),
    })
    .await
}

fn remote_entity(object: &ArkObject, title: &str, hlc: &str, deleted: bool) -> SyncEntity {
    let Value::Object(mut data) = serde_json::to_value(object).unwrap() else {
        unreachable!()
    };
    data.remove("id");
    data.insert("title".to_string(), json!(title));
    SyncEntity {
        entity_type: "object".to_string(),
        id: object.id.clone(),
        data,
        hlc: hlc.to_string(),
        deleted: Some(deleted),
        origin_device_id: None,
        origin_seq: None,
    }
}

#[tokio::test]
async fn guarded_object_upsert_rejects_every_stale_identity_or_revision() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let dir = tempfile::tempdir().unwrap();
    handle_request(Request::Init {
        db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
    })
    .await
    .unwrap();

    let missing = snapshot("guarded-task").await;
    guarded_upsert(task("guarded-task", "created"), missing.clone())
        .await
        .unwrap();
    assert_eq!(
        guarded_upsert(task("guarded-task", "duplicate"), missing)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let stale_live = snapshot("guarded-task").await;
    handle_request(Request::UpsertObject {
        object: task("guarded-task", "newer"),
        expected_snapshot: None,
        device_id: Some("competing-writer".to_string()),
    })
    .await
    .unwrap();
    assert_eq!(
        guarded_upsert(task("guarded-task", "stale"), stale_live)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
    assert_eq!(
        handle_request(Request::GetObject {
            id: "guarded-task".to_string()
        })
        .await
        .unwrap()["title"],
        "newer"
    );

    let stale_delete = snapshot("guarded-task").await;
    handle_request(Request::UpsertObject {
        object: task("guarded-task", "newest"),
        expected_snapshot: None,
        device_id: Some("competing-writer".to_string()),
    })
    .await
    .unwrap();
    assert_eq!(
        handle_request(Request::DeleteObject {
            id: "guarded-task".to_string(),
            expected_snapshot: Some(stale_delete),
            device_id: Some("snapshot-test".to_string()),
        })
        .await
        .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let stale_identity = snapshot("guarded-task").await;
    with_write_tx(|conn| {
        conn.execute(
            "UPDATE objects SET type_id = 'com.kosmos.note', type_version = '1.0.0' WHERE id = ?1",
            ["guarded-task"],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        guarded_upsert(task("guarded-task", "wrong-type"), stale_identity)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    with_write_tx(|conn| {
        conn.execute(
            "UPDATE objects SET type_id = 'com.kosmos.task', type_version = '1.0.0' WHERE id = ?1",
            ["guarded-task"],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();
    let before_delete = snapshot("guarded-task").await;
    handle_request(Request::DeleteObject {
        id: "guarded-task".to_string(),
        expected_snapshot: None,
        device_id: Some("competing-writer".to_string()),
    })
    .await
    .unwrap();
    assert_eq!(
        guarded_upsert(task("guarded-task", "resurrect"), before_delete)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let deleted = snapshot("guarded-task").await;
    handle_request(Request::UpsertObject {
        object: task("guarded-task", "recreated"),
        expected_snapshot: None,
        device_id: Some("competing-writer".to_string()),
    })
    .await
    .unwrap();
    assert_eq!(
        guarded_upsert(task("guarded-task", "stale-delete"), deleted)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
}

#[tokio::test]
async fn local_object_revision_blocks_older_remote_upsert_and_delete() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let dir = tempfile::tempdir().unwrap();
    handle_request(Request::Init {
        db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
    })
    .await
    .unwrap();
    handle_request(Request::UpsertObject {
        object: task("remote-guarded-task", "local"),
        expected_snapshot: None,
        device_id: Some("local-writer".to_string()),
    })
    .await
    .unwrap();

    let backend = SqliteStorageBackend::new(get_shared_conn().unwrap());
    let local = with_conn(|conn| {
        db::get_object(conn, "remote-guarded-task")?.ok_or("object missing".to_string())
    })
    .unwrap();
    let before_update = snapshot("remote-guarded-task").await;
    backend
        .apply_entity(&remote_entity(
            &local,
            "older remote",
            "2000-01-01T00:00:00.000Z:000001:remote",
            false,
        ))
        .await
        .unwrap();
    assert_eq!(
        with_conn(|conn| db::get_object(conn, "remote-guarded-task"))
            .unwrap()
            .unwrap()
            .title,
        "local"
    );
    guarded_upsert(task("remote-guarded-task", "guarded"), before_update)
        .await
        .unwrap();

    let before_delete = snapshot("remote-guarded-task").await;
    let local = with_conn(|conn| {
        db::get_object(conn, "remote-guarded-task")?.ok_or("object missing".to_string())
    })
    .unwrap();
    backend
        .apply_entity(&remote_entity(
            &local,
            "",
            "2000-01-01T00:00:00.000Z:000002:remote",
            true,
        ))
        .await
        .unwrap();
    handle_request(Request::DeleteObject {
        id: "remote-guarded-task".to_string(),
        expected_snapshot: Some(before_delete),
        device_id: Some("local-writer".to_string()),
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn newer_remote_upsert_and_delete_invalidate_guarded_writes() {
    let _guard = TEST_DB_MUTEX.lock().await;
    let dir = tempfile::tempdir().unwrap();
    handle_request(Request::Init {
        db_path: dir.path().join("ark.db").to_string_lossy().to_string(),
    })
    .await
    .unwrap();
    handle_request(Request::UpsertObject {
        object: task("remote-race-task", "local"),
        expected_snapshot: None,
        device_id: Some("local-writer".to_string()),
    })
    .await
    .unwrap();

    let backend = SqliteStorageBackend::new(get_shared_conn().unwrap());
    let local = with_conn(|conn| {
        db::get_object(conn, "remote-race-task")?.ok_or("object missing".to_string())
    })
    .unwrap();
    let before_remote_update = snapshot("remote-race-task").await;
    backend
        .apply_entity(&remote_entity(
            &local,
            "newer remote",
            "2999-01-01T00:00:00.000Z:000001:remote",
            false,
        ))
        .await
        .unwrap();
    assert_eq!(
        guarded_upsert(task("remote-race-task", "stale"), before_remote_update)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let remote = with_conn(|conn| {
        db::get_object(conn, "remote-race-task")?.ok_or("object missing".to_string())
    })
    .unwrap();
    let before_remote_delete = snapshot("remote-race-task").await;
    backend
        .apply_entity(&remote_entity(
            &remote,
            "",
            "2999-01-01T00:00:00.000Z:000002:remote",
            true,
        ))
        .await
        .unwrap();
    assert_eq!(
        handle_request(Request::DeleteObject {
            id: "remote-race-task".to_string(),
            expected_snapshot: Some(before_remote_delete),
            device_id: Some("local-writer".to_string()),
        })
        .await
        .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
}
