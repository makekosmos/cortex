use super::*;

#[tokio::test]
async fn authenticated_full_snapshot_does_not_erase_saved_task_merge_variants() {
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

    let operation =
        |suffix, parents: &[char], title: &str| crate::domain::task_merge::TaskMergeOperation {
            id: crate::domain::task_merge::OperationId::parse(format!(
                "{}{}",
                "0".repeat(31),
                suffix
            ))
            .unwrap(),
            object_id: "snapshot-with-merge-history".into(),
            unit: crate::domain::task_merge::TaskMergeUnit::Title,
            parents: parents
                .iter()
                .map(|parent| {
                    crate::domain::task_merge::OperationId::parse(format!(
                        "{}{}",
                        "0".repeat(31),
                        parent
                    ))
                    .unwrap()
                })
                .collect(),
            value: crate::domain::task_merge::TaskMergeValue::Field(json!(title)),
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

    let object = with_conn(&state, |conn| {
        crate::canonical_types::ingress::prepare_object(
            conn,
            task("snapshot-with-merge-history", "legacy snapshot"),
        )
        .map_err(|error| error.to_string())
    })
    .unwrap();
    let snapshot = full_task_entity(&object, "2990-01-01T00:00:00.000Z:000001:inbound-peer");
    assert_eq!(
        apply_authenticated(&state, &snapshot, "merge-history-snapshot").await,
        crate::application::sync_storage::InboundApplyOutcome::Applied
    );

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
    assert_eq!(
        with_conn(&state, |conn| Ok(db::get_object(conn, &object.id)?
            .unwrap()
            .title))
        .unwrap(),
        "legacy snapshot"
    );
}
