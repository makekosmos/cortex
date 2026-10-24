use super::*;

async fn snapshot(state: &Arc<ServiceState>, id: &str) -> ObjectWriteSnapshot {
    serde_json::from_value(
        handle_request(
            state,
            Request::GetObjectWriteSnapshot { id: id.to_string() },
        )
        .await
        .unwrap(),
    )
    .unwrap()
}

async fn object(state: &Arc<ServiceState>, id: &str) -> ark_domain::types::ArkObject {
    serde_json::from_value(
        handle_request(state, Request::GetObject { id: id.to_string() })
            .await
            .unwrap(),
    )
    .unwrap()
}

fn task(id: &str, title: &str) -> ArkObjectWrite {
    let mut object = canonical_task_object(Some("0.1.0"), canonical_task_props());
    object.id = id.to_string();
    object.title = title.to_string();
    object
}

fn project(id: &str, title: &str) -> ArkObjectWrite {
    ArkObjectWrite {
        id: id.to_string(),
        type_id: "core.project".to_string(),
        type_version: Some("0.1.0".to_string()),
        title: title.to_string(),
        content_json: json!({
            "type": "doc",
            "content": [{ "type": "paragraph" }]
        }),
        props_json: json!({
            "status": "active",
            "scheduledAt": null,
            "dueAt": null,
            "color": null,
            "extensions": {}
        }),
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        deleted_at: None,
    }
}

async fn guarded_upsert(
    state: &Arc<ServiceState>,
    object: ArkObjectWrite,
    expected_snapshot: ObjectWriteSnapshot,
) -> Result<Value, String> {
    handle_request(
        state,
        Request::UpsertObject {
            object,
            expected_snapshot: Some(expected_snapshot),
            operation_id: None,
            device_id: Some("snapshot-test".to_string()),
        },
    )
    .await
}

fn remote_entity(object: &ArkObject, title: &str, hlc: &str, deleted: bool) -> SyncEntity {
    let value = serde_json::to_value(object).unwrap();
    let Value::Object(mut data) = value else {
        panic!("expected JSON object, got {value:?}")
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
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let storage = ark_store::test_support::ObjectRevisionFixture::open(&fixture.db_path()).unwrap();
    initialize_test_database(&state, dir.path().join("ark.db").to_string_lossy().to_string(),)
    .await
    .unwrap();

    let missing = snapshot(&state, "guarded-task").await;
    guarded_upsert(&state, task("guarded-task", "created"), missing.clone())
        .await
        .unwrap();
    assert_eq!(
        guarded_upsert(&state, task("guarded-task", "duplicate"), missing)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let stale_live = snapshot(&state, "guarded-task").await;
    assert_eq!(
        handle_request(
            &state,
            Request::UpsertObject {
                object: task("guarded-task", "missing-precondition"),
                expected_snapshot: None,
                operation_id: None,
                device_id: Some("competing-writer".to_string()),
            },
        )
        .await
        .unwrap_err(),
        "object_conflict:expected_snapshot_required"
    );
    handle_request(
        &state,
        Request::UpsertObject {
            object: task("guarded-task", "newer"),
            expected_snapshot: Some(stale_live.clone()),
            operation_id: None,
            device_id: Some("competing-writer".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        guarded_upsert(&state, task("guarded-task", "stale"), stale_live)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
    assert_eq!(
        handle_request(
            &state,
            Request::GetObject {
                id: "guarded-task".to_string()
            }
        )
        .await
        .unwrap()["title"],
        "newer"
    );

    let stale_delete = snapshot(&state, "guarded-task").await;
    handle_request(
        &state,
        Request::UpsertObject {
            object: task("guarded-task", "newest"),
            expected_snapshot: Some(stale_delete.clone()),
            operation_id: None,
            device_id: Some("competing-writer".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        handle_request(
            &state,
            Request::DeleteObject {
                id: "guarded-task".to_string(),
                expected_snapshot: Some(stale_delete),
                operation_id: None,
                device_id: Some("snapshot-test".to_string()),
            }
        )
        .await
        .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let stale_identity = snapshot(&state, "guarded-task").await;
    storage
        .set_object_type_identity("guarded-task", "core.note", "0.1.0")
        .unwrap();
    assert_eq!(
        guarded_upsert(&state, task("guarded-task", "wrong-type"), stale_identity)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    storage
        .set_object_type_identity("guarded-task", "core.task", "0.1.0")
        .unwrap();
    let before_delete = snapshot(&state, "guarded-task").await;
    handle_request(
        &state,
        Request::DeleteObject {
            id: "guarded-task".to_string(),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("competing-writer".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        guarded_upsert(&state, task("guarded-task", "resurrect"), before_delete)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let deleted = snapshot(&state, "guarded-task").await;
    handle_request(
        &state,
        Request::UpsertObject {
            object: task("guarded-task", "recreated"),
            expected_snapshot: Some(deleted.clone()),
            operation_id: None,
            device_id: Some("competing-writer".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        guarded_upsert(&state, task("guarded-task", "stale-delete"), deleted)
            .await
            .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
}

#[tokio::test]
async fn local_object_revision_blocks_older_remote_upsert_and_delete() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    initialize_test_database(&state, dir.path().join("ark.db").to_string_lossy().to_string(),)
    .await
    .unwrap();
    handle_request(
        &state,
        Request::UpsertObject {
            object: project("remote-guarded-object", "local"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("local-writer".to_string()),
        },
    )
    .await
    .unwrap();

    let backend = SqliteStorageBackend::with_db_handle(get_db_handle(&state).unwrap());
    let local = object(&state, "remote-guarded-object").await;
    let before_update = snapshot(&state, "remote-guarded-object").await;
    backend
        .apply_entity(&remote_entity(
            &local,
            "older remote",
            "2000-01-01T00:00:00.000Z:000001:remote",
            false,
        ))
        .await
        .unwrap();
    assert_eq!(object(&state, "remote-guarded-object").await.title, "local");
    guarded_upsert(
        &state,
        project("remote-guarded-object", "guarded"),
        before_update,
    )
    .await
    .unwrap();

    let before_delete = snapshot(&state, "remote-guarded-object").await;
    let local = object(&state, "remote-guarded-object").await;
    backend
        .apply_entity(&remote_entity(
            &local,
            "",
            "2000-01-01T00:00:00.000Z:000002:remote",
            true,
        ))
        .await
        .unwrap();
    handle_request(
        &state,
        Request::DeleteObject {
            id: "remote-guarded-object".to_string(),
            expected_snapshot: Some(before_delete),
            operation_id: None,
            device_id: Some("local-writer".to_string()),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn newer_remote_upsert_and_delete_invalidate_guarded_writes() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    initialize_test_database(&state, dir.path().join("ark.db").to_string_lossy().to_string(),)
    .await
    .unwrap();
    handle_request(
        &state,
        Request::UpsertObject {
            object: project("remote-race-object", "local"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("local-writer".to_string()),
        },
    )
    .await
    .unwrap();

    let backend = SqliteStorageBackend::with_db_handle(get_db_handle(&state).unwrap());
    let local = object(&state, "remote-race-object").await;
    let before_remote_update = snapshot(&state, "remote-race-object").await;
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
        guarded_upsert(
            &state,
            project("remote-race-object", "stale"),
            before_remote_update
        )
        .await
        .unwrap_err(),
        "object_conflict:stale_snapshot"
    );

    let remote = object(&state, "remote-race-object").await;
    let before_remote_delete = snapshot(&state, "remote-race-object").await;
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
        handle_request(
            &state,
            Request::DeleteObject {
                id: "remote-race-object".to_string(),
                expected_snapshot: Some(before_remote_delete),
                operation_id: None,
                device_id: Some("local-writer".to_string()),
            }
        )
        .await
        .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
}
