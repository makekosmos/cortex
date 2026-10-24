use super::*;

#[tokio::test]
async fn external_ref_request_keeps_wire_shape_and_rolls_back_failed_update() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.db_path();
    initialize_test_database(&state, db_path.to_string_lossy().into_owned(),)
    .await
    .unwrap();

    let mut object = canonical_task_object(Some("0.1.0"), canonical_task_props());
    object.id = "external-ref-object".into();
    handle_request(
        &state,
        Request::UpsertObject {
            object,
            expected_snapshot: None,
            operation_id: None,
            device_id: None,
        },
    )
    .await
    .unwrap();

    let request: Request = serde_json::from_value(json!({
        "operation": "external_refs.upsert",
        "connectorId": "drive",
        "accountId": "personal",
        "externalType": "file",
        "externalId": "file-1",
        "objectId": "external-ref-object",
        "revision": "r1",
        "hash": "abc",
        "state": "clean"
    }))
    .unwrap();
    assert_eq!(handle_request(&state, request).await.unwrap(), json!(true));

    let stored = ark_store::test_support::ExternalRefFixture::inspect(&db_path)
        .unwrap()
        .expect("external reference is persisted");
    assert_eq!(
        stored,
        ark_store::test_support::ExternalRefSnapshot {
            connector_id: "drive".into(),
            account_id: "personal".into(),
            external_type: "file".into(),
            external_id: "file-1".into(),
            object_id: "external-ref-object".into(),
            revision: Some("r1".into()),
            content_hash: Some("abc".into()),
            conflict_state: "clean".into(),
        }
    );

    ark_store::test_support::ExternalRefFixture::reject_update_commit(&db_path).unwrap();

    let failed_update: Request = serde_json::from_value(json!({
        "operation": "external_refs.upsert",
        "connectorId": "drive",
        "accountId": "personal",
        "externalType": "file",
        "externalId": "file-1",
        "objectId": "external-ref-object",
        "revision": "r2",
        "hash": "def",
        "state": "clean"
    }))
    .unwrap();
    let error = handle_request(&state, failed_update).await.unwrap_err();
    assert!(error.contains("FOREIGN KEY constraint failed"), "{error}");

    let stored = ark_store::test_support::ExternalRefFixture::inspect(&db_path)
        .unwrap()
        .expect("failed update must preserve the reference");
    assert_eq!(stored.revision.as_deref(), Some("r1"));
    assert_eq!(stored.content_hash.as_deref(), Some("abc"));
}
