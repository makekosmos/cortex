use super::*;
use ark_domain::task_merge::{OperationId, TaskMergeOperation, TaskMergeUnit, TaskMergeValue};

fn operation(id: char, parents: &[char], value: &str) -> TaskMergeOperation {
    merge_operation(
        id,
        TaskMergeUnit::Title,
        parents,
        TaskMergeValue::Field(json!(value)),
    )
}

fn merge_operation(
    id: char,
    unit: TaskMergeUnit,
    parents: &[char],
    value: TaskMergeValue,
) -> TaskMergeOperation {
    TaskMergeOperation {
        id: OperationId::parse(format!("{}{}", "0".repeat(31), id)).unwrap(),
        object_id: "service-task".into(),
        unit,
        parents: parents
            .iter()
            .map(|id| OperationId::parse(format!("{}{}", "0".repeat(31), id)).unwrap())
            .collect(),
        observed_heads: parents
            .iter()
            .map(|id| OperationId::parse(format!("{}{}", "0".repeat(31), id)).unwrap())
            .collect(),
        hlc: None,
        value,
    }
}

async fn init(state: &Arc<ServiceState>, path: &std::path::Path) {
    handle_request(
        state,
        Request::Init {
            db_path: path.to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn task_merge_public_requests_preserve_conflicts_and_reject_stale_resolution() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    init(&state, &fixture.db_path()).await;

    let base = handle_request(
        &state,
        Request::TaskMergeAppend {
            merge_operation: operation('1', &[], "base"),
            expected_conflict_digest: None,
        },
    )
    .await
    .unwrap();
    let base_digest = base["conflictSet"]["digest"].as_str().unwrap().to_owned();

    for (id, title) in [('2', "left"), ('3', "right")] {
        handle_request(
            &state,
            Request::TaskMergeAppend {
                merge_operation: operation(id, &['1'], title),
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }

    let conflicts = handle_request(
        &state,
        Request::TaskMergeConflicts {
            object_id: "service-task".into(),
            unit: TaskMergeUnit::Title,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        conflicts["conflictSet"]["variants"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(conflicts["dependenciesPending"], false);

    let stale = handle_request(
        &state,
        Request::TaskMergeAppend {
            merge_operation: operation('4', &['2', '3'], "resolved"),
            expected_conflict_digest: Some(base_digest),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(stale, "task_merge_conflict_set_changed");
}

#[tokio::test]
async fn sqlite_service_replicas_converge_after_reordered_merge_delivery() {
    let left = service_fixture();
    let right = service_fixture();
    init(&left.state, &left.db_path()).await;
    init(&right.state, &right.db_path()).await;

    let title_base = operation('1', &[], "base title");
    let deadline_base = merge_operation(
        '4',
        TaskMergeUnit::Deadline,
        &[],
        TaskMergeValue::Field(json!("2030-01-01")),
    );
    for state in [&left.state, &right.state] {
        for merge_operation in [title_base.clone(), deadline_base.clone()] {
            handle_request(
                state,
                Request::TaskMergeAppend {
                    merge_operation,
                    expected_conflict_digest: None,
                },
            )
            .await
            .unwrap();
        }
    }

    let title_left = operation('2', &['1'], "left title");
    let title_right = operation('3', &['1'], "right title");
    let deadline_edit = merge_operation(
        '5',
        TaskMergeUnit::Deadline,
        &['4'],
        TaskMergeValue::Field(json!("2030-02-01")),
    );
    for merge_operation in [
        title_left.clone(),
        title_right.clone(),
        deadline_edit.clone(),
    ] {
        handle_request(
            &left.state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }
    for merge_operation in [deadline_edit, title_right, title_left] {
        handle_request(
            &right.state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }

    let title_sets = [
        handle_request(
            &left.state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::Title,
            },
        )
        .await
        .unwrap(),
        handle_request(
            &right.state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::Title,
            },
        )
        .await
        .unwrap(),
    ];
    assert_eq!(title_sets[0]["conflictSet"], title_sets[1]["conflictSet"]);
    assert_eq!(
        title_sets[0]["conflictSet"]["commonAncestors"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        title_sets[0]["conflictSet"]["variants"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    for state in [&left.state, &right.state] {
        let deadline_set = handle_request(
            state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::Deadline,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            deadline_set["conflictSet"]["variants"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[tokio::test]
async fn sqlite_service_replicas_preserve_a_concurrent_delete_and_edit() {
    let left = service_fixture();
    let right = service_fixture();
    init(&left.state, &left.db_path()).await;
    init(&right.state, &right.db_path()).await;

    let title_base = operation('1', &[], "before edit");
    let existence_base = merge_operation(
        '2',
        TaskMergeUnit::ObjectExistence,
        &[],
        TaskMergeValue::ObjectExists,
    );
    for state in [&left.state, &right.state] {
        for merge_operation in [title_base.clone(), existence_base.clone()] {
            handle_request(
                state,
                Request::TaskMergeAppend {
                    merge_operation,
                    expected_conflict_digest: None,
                },
            )
            .await
            .unwrap();
        }
    }

    let deletion = merge_operation(
        '3',
        TaskMergeUnit::ObjectExistence,
        &['2'],
        TaskMergeValue::ObjectDeleted {
            deleted_at: "2026-10-17T10:00:00Z".into(),
        },
    );
    let title_edit = operation('4', &['1'], "edited concurrently");
    let edit_kept_object_alive = merge_operation(
        '5',
        TaskMergeUnit::ObjectExistence,
        &['2'],
        TaskMergeValue::ObjectExists,
    );

    for merge_operation in [
        deletion.clone(),
        title_edit.clone(),
        edit_kept_object_alive.clone(),
    ] {
        handle_request(
            &left.state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }
    for merge_operation in [title_edit, edit_kept_object_alive, deletion] {
        handle_request(
            &right.state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }

    let existence_sets = [
        handle_request(
            &left.state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::ObjectExistence,
            },
        )
        .await
        .unwrap(),
        handle_request(
            &right.state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::ObjectExistence,
            },
        )
        .await
        .unwrap(),
    ];
    assert_eq!(
        existence_sets[0]["conflictSet"],
        existence_sets[1]["conflictSet"]
    );
    assert_eq!(
        existence_sets[0]["conflictSet"]["commonAncestors"][0]["id"],
        existence_base.id.as_str()
    );
    assert_eq!(
        existence_sets[0]["conflictSet"]["variants"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    for state in [&left.state, &right.state] {
        let title_set = handle_request(
            state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::Title,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            title_set["conflictSet"]["variants"][0]["value"]["value"],
            "edited concurrently"
        );
        assert_eq!(
            title_set["conflictSet"]["variants"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[tokio::test]
async fn sqlite_service_replicas_keep_rich_text_documents_as_atomic_variants() {
    let left = service_fixture();
    let right = service_fixture();
    init(&left.state, &left.db_path()).await;
    init(&right.state, &right.db_path()).await;

    let base = merge_operation(
        '6',
        TaskMergeUnit::Content,
        &[],
        TaskMergeValue::Field(json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "base"}]}]
        })),
    );
    for state in [&left.state, &right.state] {
        handle_request(
            state,
            Request::TaskMergeAppend {
                merge_operation: base.clone(),
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }

    let left_document = json!({
        "type": "doc",
        "content": [{"type": "paragraph", "content": [{"type": "text", "text": "left edit"}]}]
    });
    let right_document = json!({
        "type": "doc",
        "content": [{"type": "paragraph", "content": [{"type": "text", "text": "right edit"}]}]
    });
    let left_edit = merge_operation(
        '7',
        TaskMergeUnit::Content,
        &['6'],
        TaskMergeValue::Field(left_document.clone()),
    );
    let right_edit = merge_operation(
        '8',
        TaskMergeUnit::Content,
        &['6'],
        TaskMergeValue::Field(right_document.clone()),
    );
    for merge_operation in [left_edit.clone(), right_edit.clone()] {
        handle_request(
            &left.state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }
    for merge_operation in [right_edit, left_edit] {
        handle_request(
            &right.state,
            Request::TaskMergeAppend {
                merge_operation,
                expected_conflict_digest: None,
            },
        )
        .await
        .unwrap();
    }

    let sets = [
        handle_request(
            &left.state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::Content,
            },
        )
        .await
        .unwrap(),
        handle_request(
            &right.state,
            Request::TaskMergeConflicts {
                object_id: "service-task".into(),
                unit: TaskMergeUnit::Content,
            },
        )
        .await
        .unwrap(),
    ];
    assert_eq!(sets[0]["conflictSet"], sets[1]["conflictSet"]);
    assert_eq!(
        sets[0]["conflictSet"]["commonAncestors"][0]["id"],
        base.id.as_str()
    );
    assert_eq!(
        sets[0]["conflictSet"]["variants"].as_array().unwrap().len(),
        2
    );
    let variants = sets[0]["conflictSet"]["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variant| variant["value"]["value"].clone())
        .collect::<Vec<_>>();
    assert!(variants.contains(&left_document));
    assert!(variants.contains(&right_document));
}

#[test]
fn task_merge_append_request_deserializes_without_colliding_with_operation_tag() {
    let request: Request = serde_json::from_value(json!({
        "operation": "task.merge.append",
        "mergeOperation": {
            "id": "00000000000000000000000000000001",
            "objectId": "service-task",
            "unit": "title",
            "parents": [],
            "value": {"kind": "field", "value": "title"}
        }
    }))
    .unwrap();
    assert!(matches!(request, Request::TaskMergeAppend { .. }));
}
