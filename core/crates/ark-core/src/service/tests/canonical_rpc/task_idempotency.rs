use super::*;

#[tokio::test]
async fn canonical_upsert_object_rpc_persists_registered_identity() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    initialize_test_database(&state, db_path.to_string_lossy().to_string())
        .await
        .unwrap();
    handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(Some("0.1.0"), canonical_task_props()),
            expected_snapshot: None,
            operation_id: Some("canonical-task-ingress-replay".to_string()),
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await
    .unwrap();
    let first_task_revision = handle_request(
        &state,
        Request::GetObjectWriteSnapshot {
            id: "canonical-task-ingress".to_string(),
        },
    )
    .await
    .unwrap()["revision"]
        .as_str()
        .unwrap()
        .to_string();
    handle_request(
        &state,
        Request::UpsertObject {
            object: canonical_task_object(Some("0.1.0"), canonical_task_props()),
            expected_snapshot: None,
            operation_id: Some("canonical-task-ingress-replay".to_string()),
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await
    .expect("retry returns its stored receipt");
    let mut conflicting_task = canonical_task_object(Some("0.1.0"), canonical_task_props());
    conflicting_task.title = "Different task".into();
    let conflict = handle_request(
        &state,
        Request::UpsertObject {
            object: conflicting_task,
            expected_snapshot: None,
            operation_id: Some("canonical-task-ingress-replay".to_string()),
            device_id: Some("device-canonical".to_string()),
        },
    )
    .await
    .expect_err("operation id reuse with another payload is rejected");
    assert!(conflict.to_string().contains("different command payload"));
    let object = handle_request(
        &state,
        Request::GetObject {
            id: "canonical-task-ingress".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(object["typeId"], "core.task");
    assert_eq!(object["typeVersion"], "0.1.0");
    assert_eq!(object["propsJson"]["planning"]["kind"], "anytime");
    let receipts = ark_store::test_support::CanonicalTaskIdempotencyFixture::inspect(
        &fixture.db_path(),
        "canonical-task-ingress",
        "canonical-task-ingress-replay",
    )
    .unwrap();
    assert_eq!(receipts.entity_write_receipts, 1);
    assert_eq!(receipts.operation_receipts, 1);
    let snapshot = handle_request(
        &state,
        Request::GetObjectWriteSnapshot {
            id: "canonical-task-ingress".to_string(),
        },
    )
    .await
    .unwrap();
    let task_revision = snapshot["revision"].as_str().unwrap();
    assert_eq!(task_revision, first_task_revision);
    fixture.shutdown().await;
}
