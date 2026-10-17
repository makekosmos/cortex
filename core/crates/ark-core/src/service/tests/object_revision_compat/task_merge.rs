use super::*;

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

    let operation = |suffix, parents: &[char], title: &str| {
        let parents = parents
            .iter()
            .map(|parent| {
                crate::domain::task_merge::OperationId::parse(format!(
                    "{}{}",
                    "0".repeat(31),
                    parent
                ))
                .unwrap()
            })
            .collect::<Vec<_>>();
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
            value: crate::domain::task_merge::TaskMergeValue::Field(json!(title)),
        }
    };
    for merge_operation in [
        operation('1', &[], "base"),
        operation('2', &['1'], "left edit"),
        operation('3', &['1'], "right edit"),
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
    assert_eq!(local_error, "task_merge_operation_mode_requires_upgrade");

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
            "original local value"
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
        assert_eq!(operations, 3);
        Ok(())
    })
    .unwrap();
}
