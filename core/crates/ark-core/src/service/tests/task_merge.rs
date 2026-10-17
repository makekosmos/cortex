use super::*;
use crate::domain::task_merge::{OperationId, TaskMergeOperation, TaskMergeUnit, TaskMergeValue};

fn operation(id: char, parents: &[char], value: &str) -> TaskMergeOperation {
    TaskMergeOperation {
        id: OperationId::parse(format!("{}{}", "0".repeat(31), id)).unwrap(),
        object_id: "service-task".into(),
        unit: TaskMergeUnit::Title,
        parents: parents
            .iter()
            .map(|id| OperationId::parse(format!("{}{}", "0".repeat(31), id)).unwrap())
            .collect(),
        value: TaskMergeValue::Field(json!(value)),
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
