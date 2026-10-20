use super::*;

#[tokio::test]
async fn canonical_upsert_object_rpc_persists_registered_identity() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
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
    let first_task_revision = with_conn(&state, |conn| {
        conn.query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id = 'canonical-task-ingress'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| error.to_string())
    })
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
    let object = with_conn(&state, |conn| {
        ark_store::objects::get_object(conn, "canonical-task-ingress")
    })
    .unwrap()
    .unwrap();
    assert_eq!(object.type_id, "core.task");
    assert_eq!(object.type_version, "0.1.0");
    assert_eq!(object.props_json["planning"]["kind"], "anytime");
    with_conn(&state, |conn| {
        let receipt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts
                 WHERE entity_kind = 'object' AND entity_id = 'canonical-task-ingress'
                   AND representation_kind = 'full' AND outcome = 'applied'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(receipt_count, 1);
        let operation_receipt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ark_operation_receipts
                 WHERE principal = 'engine' AND operation_id = 'canonical-task-ingress-replay'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(operation_receipt_count, 1);
        let task_revision: String = conn
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id = 'canonical-task-ingress'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(task_revision, first_task_revision);
        Ok(())
    })
    .unwrap();
    fixture.shutdown().await;
}
