use super::*;

#[tokio::test]
async fn editing_legacy_task_builds_mutation_from_the_new_bootstrap_baseline() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let id = "legacy-task-bootstrap";
    let storage =
        ark_store::test_support::LegacyTaskMergeFixture::open(&fixture.db_path()).unwrap();
    storage
        .seed_legacy_task(task(id, "before"), "2026-10-01T00:00:00.000Z:000001:legacy")
        .unwrap();

    let snapshot = object_snapshot(&state, id).await;
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "after"),
            expected_snapshot: Some(snapshot),
            operation_id: None,
            device_id: Some("legacy-mutation-device".into()),
        },
    )
    .await
    .unwrap();

    let stored = storage.inspect(id).unwrap();
    assert_eq!(
        stored.bootstrap.map(|bootstrap| bootstrap.object.title),
        Some("before".into())
    );
    assert_eq!(
        stored.operation_count,
        ark_domain::task_merge::TaskMergeUnit::ALL.len() as i64 + 1
    );
    assert!(stored
        .title_conflicts
        .is_some_and(|set| set.variants.iter().any(|operation| {
            operation.value == ark_domain::task_merge::TaskMergeValue::Field(json!("after"))
        })));
    let object = handle_request(&state, Request::GetObject { id: id.into() })
        .await
        .unwrap();
    assert_eq!(object["title"], "after");
}

#[tokio::test]
async fn deleting_legacy_task_uses_the_same_bootstrap_conflict_set_it_persists() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let id = "legacy-task-delete-bootstrap";
    let storage =
        ark_store::test_support::LegacyTaskMergeFixture::open(&fixture.db_path()).unwrap();
    storage
        .seed_legacy_task(task(id, "before"), "2026-10-01T00:00:00.000Z:000001:legacy")
        .unwrap();

    let snapshot = object_snapshot(&state, id).await;
    handle_request(
        &state,
        Request::DeleteObject {
            id: id.into(),
            expected_snapshot: Some(snapshot),
            operation_id: None,
            device_id: Some("legacy-delete-device".into()),
        },
    )
    .await
    .unwrap();

    let stored = storage.inspect(id).unwrap();
    assert!(stored.object.is_none());
    assert!(stored.bootstrap.is_some());
    assert!(
        stored.existence_operations.iter().any(|operation| {
            matches!(
                operation.value,
                ark_domain::task_merge::TaskMergeValue::ObjectHardDeleted { .. }
            )
        }),
        "persisted object-existence operations: {:?}",
        stored.existence_operations
    );
}

#[tokio::test]
async fn failed_local_task_journal_write_rolls_back_the_complete_mutation() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();

    let storage =
        ark_store::test_support::LegacyTaskMergeFixture::open(&fixture.db_path()).unwrap();
    storage
        .seed_legacy_task(
            task("rollback-task-mutation", "before"),
            "2026-10-01T00:00:00.000Z:000001:legacy",
        )
        .unwrap();
    storage.reject_operation_journal_writes().unwrap();

    assert!(handle_request(
        &state,
        Request::UpsertObject {
            object: task("rollback-task-mutation", "failed"),
            expected_snapshot: Some(object_snapshot(&state, "rollback-task-mutation").await),
            operation_id: None,
            device_id: Some("rollback-test-device".into()),
        },
    )
    .await
    .is_err());

    let stored = storage.inspect("rollback-task-mutation").unwrap();
    assert_eq!(
        stored.object.as_ref().map(|object| object.title.as_str()),
        Some("before")
    );
    assert!(stored.bootstrap.is_none());
    assert_eq!(stored.operation_count, 0);
    assert_eq!(stored.journal_count, 0);
    assert_eq!(stored.link_count, 0);
    assert_eq!(stored.tombstone_count, 0);
    assert_eq!(stored.revision_count, 1);
    assert_eq!(stored.committed_feed_count, 0);
    assert!(!stored.version_vector_has_object);
    let object = handle_request(
        &state,
        Request::GetObject {
            id: "rollback-task-mutation".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(object["title"], "before");
}
