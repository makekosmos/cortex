use super::*;

async fn object_snapshot(
    state: &Arc<ServiceState>,
    id: &str,
) -> crate::domain::object_write::ObjectWriteSnapshot {
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
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
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

    let bootstrap_id = with_conn(&state, |connection| {
        connection
            .query_row(
                "SELECT bootstrap_id FROM task_merge_bootstraps WHERE object_id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();
    let baseline_ids = with_conn(&state, |connection| {
        let mut statement = connection
            .prepare("SELECT operation_id FROM task_merge_operations WHERE object_id = ?1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([id], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<std::collections::BTreeSet<_>, _>>()
            .map_err(|error| error.to_string())
    })
    .unwrap();
    assert_eq!(
        baseline_ids.len(),
        crate::domain::task_merge::TaskMergeUnit::ALL.len()
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

    let stored_operations = with_conn(&state, |connection| {
        let mut statement = connection
            .prepare(
                "SELECT operation_json FROM task_merge_operations
                 WHERE object_id = ?1 AND unit IN ('\"title\"', '\"deadline\"')",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([id], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            serde_json::from_str::<crate::domain::task_merge::TaskMergeOperation>(
                &row.map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()
    })
    .unwrap();
    let local_operations = stored_operations
        .into_iter()
        .filter(|operation| !baseline_ids.contains(operation.id.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(local_operations.len(), 2);
    for operation in &local_operations {
        assert_eq!(operation.parents.len(), 1);
        assert!(baseline_ids.contains(operation.parents[0].as_str()));
        assert!(
            operation.observed_heads.len() >= crate::domain::task_merge::TaskMergeUnit::ALL.len()
        );
        assert!(baseline_ids.iter().all(|baseline| operation
            .observed_heads
            .contains(&crate::domain::task_merge::OperationId::parse(baseline.clone()).unwrap())));
    }
    let final_object = handle_request(&state, Request::GetObject { id: id.into() })
        .await
        .unwrap();
    assert_eq!(final_object["title"], "after");
    assert_eq!(final_object["propsJson"]["deadline"], "2030-05-01");
    with_conn(&state, |connection| {
        let journal_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_journal WHERE record_id IN (
                     SELECT operation_id FROM task_merge_operations WHERE object_id = ?1
                 ) OR (record_kind = 'bootstrap' AND record_id = ?2)",
                rusqlite::params![id, bootstrap_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(journal_count, 3);
        Ok(())
    })
    .unwrap();
}

#[tokio::test]
async fn task_snapshot_token_changes_when_causal_heads_arrive() {
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
            unit: crate::domain::task_merge::TaskMergeUnit::Title,
        },
    )
    .await
    .unwrap()["conflictSet"]["variants"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = crate::domain::task_merge::OperationId::parse(base).unwrap();
    handle_request(
        &state,
        Request::TaskMergeAppend {
            merge_operation: crate::domain::task_merge::TaskMergeOperation {
                id: crate::domain::task_merge::OperationId::parse(
                    "00000000000000000000000000000001",
                )
                .unwrap(),
                object_id: id.into(),
                unit: crate::domain::task_merge::TaskMergeUnit::Title,
                parents: vec![base.clone()],
                observed_heads: vec![base],
                hlc: None,
                value: crate::domain::task_merge::TaskMergeValue::Field(json!("remote")),
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
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
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
    assert!(
        with_conn(&state, |connection| db::get_object(connection, id))
            .unwrap()
            .is_none()
    );

    let deletion_set = get_db_handle(&state)
        .unwrap()
        .query(
            db::CommandContext::engine(),
            db::TaskConflictSetQuery::new(
                id.into(),
                crate::domain::task_merge::TaskMergeUnit::ObjectExistence,
            ),
        )
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    assert!(matches!(
        deletion_set.variants[0].value,
        crate::domain::task_merge::TaskMergeValue::ObjectHardDeleted { .. }
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
    let restored = with_conn(&state, |connection| db::get_object(connection, id))
        .unwrap()
        .unwrap();
    assert_eq!(restored.title, "restored");
    let existence_set = get_db_handle(&state)
        .unwrap()
        .query(
            db::CommandContext::engine(),
            db::TaskConflictSetQuery::new(
                id.into(),
                crate::domain::task_merge::TaskMergeUnit::ObjectExistence,
            ),
        )
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    assert!(matches!(
        existence_set.variants[0].value,
        crate::domain::task_merge::TaskMergeValue::ObjectExists
    ));
}

#[tokio::test]
async fn authenticated_full_snapshot_is_rejected_for_operation_enabled_task() {
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
            unit: crate::domain::task_merge::TaskMergeUnit::Title,
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
            .map(crate::domain::task_merge::OperationId::parse)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        crate::domain::task_merge::TaskMergeOperation {
            id: crate::domain::task_merge::OperationId::parse(format!(
                "{}{}",
                "0".repeat(31),
                suffix
            ))
            .unwrap(),
            object_id: "snapshot-with-merge-history".into(),
            unit: crate::domain::task_merge::TaskMergeUnit::Title,
            parents: parents.clone(),
            observed_heads: parents,
            hlc: None,
            value: crate::domain::task_merge::TaskMergeValue::Field(json!(title)),
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
            object: task("snapshot-with-merge-history", "local legacy overwrite"),
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("merge-test-device".into()),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(local_error, "object_conflict:expected_snapshot_required");

    let object = with_conn(&state, |conn| {
        crate::canonical_types::ingress::prepare_object(
            conn,
            task("snapshot-with-merge-history", "legacy snapshot"),
        )
        .map_err(|error| error.to_string())
    })
    .unwrap();
    let snapshot = full_task_entity(&object, "2990-01-01T00:00:00.000Z:000001:inbound-peer");
    let error = SqliteStorageBackend::with_db_handle(get_db_handle(&state).unwrap())
        .apply_authenticated_entity_with_id(&snapshot, "inbound-peer", "merge-history-snapshot")
        .await
        .unwrap_err();
    assert_eq!(error, "task_merge_operation_mode_requires_upgrade");

    let conflicts = handle_request(
        &state,
        Request::TaskMergeConflicts {
            object_id: "snapshot-with-merge-history".into(),
            unit: crate::domain::task_merge::TaskMergeUnit::Title,
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
    with_conn(&state, |conn| {
        assert_eq!(
            db::get_object(conn, &object.id)?.unwrap().title,
            "left edit"
        );
        let receipts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(receipts, 1);
        let feed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(feed, 1);
        let operations: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_merge_operations WHERE object_id = ?1",
                [&object.id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(operations, 13);
        Ok(())
    })
    .unwrap();
}
