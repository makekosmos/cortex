use super::*;

async fn object_snapshot(
    state: &Arc<ServiceState>,
    id: &str,
) -> ark_domain::object_write::ObjectWriteSnapshot {
    serde_json::from_value(
        handle_request(state, Request::GetObjectWriteSnapshot { id: id.into() })
            .await
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn ordinary_task_upsert_bootstraps_then_appends_changed_units_atomically() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let id = "ordinary-task-mutation";
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "before"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("mutation-test-device".into()),
        },
    )
    .await
    .unwrap();

    let storage = ark_store::test_support::TaskMergeFixture::open(&fixture.db_path()).unwrap();
    let baseline = storage.inspect(id).unwrap();
    let bootstrap_id = baseline.bootstrap.as_ref().unwrap().id.as_str().to_owned();
    let baseline_ids = baseline
        .operations
        .iter()
        .map(|operation| operation.id.as_str().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        baseline_ids.len(),
        ark_domain::task_merge::TaskMergeUnit::ALL.len()
    );
    assert!(!baseline_ids.contains(&bootstrap_id));

    let snapshot = object_snapshot(&state, id).await;
    let mut changed = task(id, "after");
    changed.props_json["deadline"] = json!("2030-05-01");
    handle_request(
        &state,
        Request::UpsertObject {
            object: changed,
            expected_snapshot: Some(snapshot),
            operation_id: Some("ordinary-task-mutation-1".into()),
            device_id: Some("mutation-test-device".into()),
        },
    )
    .await
    .unwrap();

    let stored = storage.inspect(id).unwrap();
    let local_operations = stored
        .operations
        .into_iter()
        .filter(|operation| !baseline_ids.contains(operation.id.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(local_operations.len(), 2);
    for operation in &local_operations {
        assert_eq!(operation.parents.len(), 1);
        assert!(baseline_ids.contains(operation.parents[0].as_str()));
        assert!(operation.observed_heads.len() >= ark_domain::task_merge::TaskMergeUnit::ALL.len());
        assert!(baseline_ids.iter().all(|baseline| {
            operation
                .observed_heads
                .contains(&ark_domain::task_merge::OperationId::parse(baseline.clone()).unwrap())
        }));
    }
    let final_object = handle_request(&state, Request::GetObject { id: id.into() })
        .await
        .unwrap();
    assert_eq!(final_object["title"], "after");
    assert_eq!(final_object["propsJson"]["deadline"], "2030-05-01");
    assert_eq!(stored.journal_count, 1);
    assert_eq!(stored.operation_journal_count, 2);
}

#[tokio::test]
async fn failed_local_task_journal_write_rolls_back_the_complete_mutation() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let id = "rollback-task-mutation";
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "before"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("rollback-test-device".into()),
        },
    )
    .await
    .unwrap();

    let storage = ark_store::test_support::TaskMergeFixture::open(&fixture.db_path()).unwrap();
    let before = storage.inspect(id).unwrap();
    storage.reject_operation_journal_writes().unwrap();

    assert!(handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "failed"),
            expected_snapshot: Some(object_snapshot(&state, id).await),
            operation_id: None,
            device_id: Some("rollback-test-device".into()),
        },
    )
    .await
    .is_err());

    let after = storage.inspect(id).unwrap();
    assert_eq!(
        after.object.as_ref().map(|object| object.title.as_str()),
        Some("before")
    );
    assert_eq!(after.bootstrap, before.bootstrap);
    assert_eq!(after.operation_count, before.operation_count);
}

#[tokio::test]
async fn task_snapshot_token_changes_when_causal_heads_arrive() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let id = "task-snapshot-token";
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "before"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("mutation-test-device".into()),
        },
    )
    .await
    .unwrap();
    let old_snapshot = object_snapshot(&state, id).await;
    let base = handle_request(
        &state,
        Request::TaskMergeConflicts {
            object_id: id.into(),
            unit: ark_domain::task_merge::TaskMergeUnit::Title,
        },
    )
    .await
    .unwrap()["conflictSet"]["variants"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = ark_domain::task_merge::OperationId::parse(base).unwrap();
    handle_request(
        &state,
        Request::TaskMergeAppend {
            merge_operation: ark_domain::task_merge::TaskMergeOperation {
                id: ark_domain::task_merge::OperationId::parse("00000000000000000000000000000001")
                    .unwrap(),
                object_id: id.into(),
                unit: ark_domain::task_merge::TaskMergeUnit::Title,
                parents: vec![base.clone()],
                observed_heads: vec![base],
                hlc: None,
                value: ark_domain::task_merge::TaskMergeValue::Field(json!("remote")),
            },
            expected_conflict_digest: None,
        },
    )
    .await
    .unwrap();

    let new_snapshot = object_snapshot(&state, id).await;
    assert_ne!(old_snapshot.revision, new_snapshot.revision);
    let mut stale = task(id, "stale local edit");
    stale.props_json["priority"] = json!(1);
    assert_eq!(
        handle_request(
            &state,
            Request::UpsertObject {
                object: stale,
                expected_snapshot: Some(old_snapshot),
                operation_id: None,
                device_id: Some("mutation-test-device".into()),
            },
        )
        .await
        .unwrap_err(),
        "object_conflict:stale_snapshot"
    );
    let current = handle_request(&state, Request::GetObject { id: id.into() })
        .await
        .unwrap();
    assert_eq!(current["title"], "remote");
}

#[tokio::test]
async fn hard_delete_is_causal_and_a_later_upsert_restores_the_projection() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let id = "causal-hard-delete-restore";
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "before delete"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("mutation-test-device".into()),
        },
    )
    .await
    .unwrap();
    let before_delete = object_snapshot(&state, id).await;
    handle_request(
        &state,
        Request::DeleteObject {
            id: id.into(),
            expected_snapshot: Some(before_delete),
            operation_id: None,
            device_id: Some("mutation-test-device".into()),
        },
    )
    .await
    .unwrap();
    let deletion_set = get_db_handle(&state)
        .unwrap()
        .query_with_store(
            db::CommandContext::engine(),
            db::TaskConflictSetQuery::new(
                id.into(),
                ark_domain::task_merge::TaskMergeUnit::ObjectExistence,
            ),
        )
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    assert!(matches!(
        deletion_set.variants[0].value,
        ark_domain::task_merge::TaskMergeValue::ObjectHardDeleted { .. }
    ));

    let restore_snapshot = object_snapshot(&state, id).await;
    assert!(!restore_snapshot.exists);
    handle_request(
        &state,
        Request::UpsertObject {
            object: task(id, "restored"),
            expected_snapshot: Some(restore_snapshot),
            operation_id: None,
            device_id: Some("mutation-test-device".into()),
        },
    )
    .await
    .unwrap();
    let restored = handle_request(&state, Request::GetObject { id: id.into() })
        .await
        .unwrap();
    assert_eq!(restored["title"], "restored");
    let existence_set = get_db_handle(&state)
        .unwrap()
        .query_with_store(
            db::CommandContext::engine(),
            db::TaskConflictSetQuery::new(
                id.into(),
                ark_domain::task_merge::TaskMergeUnit::ObjectExistence,
            ),
        )
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    assert!(matches!(
        existence_set.variants[0].value,
        ark_domain::task_merge::TaskMergeValue::ObjectExists
    ));
}

#[tokio::test]
async fn authenticated_full_snapshot_is_rejected_for_operation_enabled_task() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();
    admit_inbound_peer(&state).await;

    let mut initial = canonical_task_object(Some("0.1.0"), canonical_task_props());
    initial.id = "snapshot-with-merge-history".into();
    initial.title = "original local value".into();
    handle_request(
        &state,
        Request::UpsertObject {
            object: initial,
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("merge-test-device".into()),
        },
    )
    .await
    .unwrap();

    let bootstrap_title_head = handle_request(
        &state,
        Request::TaskMergeConflicts {
            object_id: "snapshot-with-merge-history".into(),
            unit: ark_domain::task_merge::TaskMergeUnit::Title,
        },
    )
    .await
    .unwrap()["conflictSet"]["variants"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let operation = |suffix, parents: Vec<String>, title: &str| {
        let parents = parents
            .into_iter()
            .map(ark_domain::task_merge::OperationId::parse)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        ark_domain::task_merge::TaskMergeOperation {
            id: ark_domain::task_merge::OperationId::parse(format!("{}{}", "0".repeat(31), suffix))
                .unwrap(),
            object_id: "snapshot-with-merge-history".into(),
            unit: ark_domain::task_merge::TaskMergeUnit::Title,
            parents: parents.clone(),
            observed_heads: parents,
            hlc: None,
            value: ark_domain::task_merge::TaskMergeValue::Field(json!(title)),
        }
    };
    for merge_operation in [
        operation('1', vec![bootstrap_title_head], "base"),
        operation('2', vec![format!("{}1", "0".repeat(31))], "left edit"),
        operation('3', vec![format!("{}1", "0".repeat(31))], "right edit"),
    ] {
        handle_request(
            &state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }

    let local_error = handle_request(
        &state,
        Request::UpsertObject {
            object: task("snapshot-with-merge-history", "local overwrite"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("merge-test-device".into()),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(local_error, "object_conflict:expected_snapshot_required");

    let conflicts = handle_request(
        &state,
        Request::TaskMergeConflicts {
            object_id: "snapshot-with-merge-history".into(),
            unit: ark_domain::task_merge::TaskMergeUnit::Title,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        conflicts["conflictSet"]["commonAncestors"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        conflicts["conflictSet"]["variants"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let stored = ark_store::test_support::TaskMergeFixture::open(&fixture.db_path())
        .unwrap()
        .inspect("snapshot-with-merge-history")
        .unwrap();
    assert_eq!(stored.object.unwrap().title, "left edit");
    assert_eq!(stored.receipt_count, 1);
    assert_eq!(stored.committed_feed_count, 1);
    assert_eq!(stored.operation_count, 13);
}
