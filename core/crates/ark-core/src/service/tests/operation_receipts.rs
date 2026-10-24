use super::*;

fn upsert_params(title: &str) -> serde_json::Value {
    json!({
        "object": {
            "id": "operation-receipt-note",
            "typeId": "com.kosmos.note",
            "typeVersion": "1.0.0",
            "title": title,
            "contentJson": {"type":"doc","content":[]},
            "propsJson": {"description":null,"extensions":{}},
            "createdAt": "2026-10-16T00:00:00.000Z",
            "updatedAt": "2026-10-16T00:00:00.000Z",
            "deletedAt": null
        },
        "operationId": "json-operation-1",
        "deviceId": "request-device-is-not-principal"
    })
}

fn upsert_request(title: &str) -> Request {
    let mut envelope = upsert_params(title).as_object().unwrap().clone();
    envelope.insert("operation".into(), json!("upsert_object"));
    serde_json::from_value(serde_json::Value::Object(envelope)).unwrap()
}

async fn write_snapshot(state: &Arc<ServiceState>) -> serde_json::Value {
    handle_request(
        state,
        Request::GetObjectWriteSnapshot {
            id: "operation-receipt-note".into(),
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn object_mutation_json_replays_same_result_and_rejects_operation_id_reuse() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned(),)
    .await
    .unwrap();
    ark_store::test_support::ObjectMutationReplayFixture::install_write_probe(&fixture.db_path())
        .expect("mutation counter triggers install");

    let (left, right) = tokio::join!(
        handle_request(&state, upsert_request("Note")),
        handle_request(&state, upsert_request("Note")),
    );
    assert_eq!(left.unwrap(), json!(true));
    assert_eq!(right.unwrap(), json!(true));
    let after_first_write = ark_store::test_support::ObjectMutationReplayFixture::inspect(
        &fixture.db_path(),
        "operation-receipt-note",
        "json-operation-1",
        "unused-delete-operation",
    )
    .expect("receipt and object state reads");
    assert_eq!(after_first_write.upsert_receipts, 1, "one receipt commits");
    assert_eq!(
        ark_store::test_support::ObjectMutationReplayFixture::write_count(&fixture.db_path())
            .expect("mutation count reads"),
        1,
        "concurrent duplicates execute the object mutation only once"
    );
    let snapshot_after_first_write = write_snapshot(&state).await;
    assert_eq!(
        handle_request(&state, upsert_request("Note"))
            .await
            .unwrap(),
        json!(true)
    );
    assert_eq!(write_snapshot(&state).await, snapshot_after_first_write);
    assert!(handle_request(&state, upsert_request("Different payload"))
        .await
        .unwrap_err()
        .contains("different command payload"));
    assert_eq!(
        write_snapshot(&state).await,
        snapshot_after_first_write,
        "a conflicting payload does not mutate the object"
    );
    assert_eq!(
        ark_store::test_support::ObjectMutationReplayFixture::inspect(
            &fixture.db_path(),
            "operation-receipt-note",
            "json-operation-1",
            "unused-delete-operation",
        )
        .expect("receipt state reads")
        .upsert_receipts,
        1,
        "a conflicting payload does not create another receipt"
    );
    assert_eq!(
        ark_store::test_support::ObjectMutationReplayFixture::write_count(&fixture.db_path())
            .expect("mutation count reads"),
        1,
        "a conflicting payload does not execute another object mutation"
    );
}

#[tokio::test]
async fn production_store_replays_receipt_written_by_the_previous_core_executor() {
    let fixture = service_fixture();
    initialize_test_database(&fixture.state, fixture.db_path().to_string_lossy().into_owned(),)
    .await
    .unwrap();
    handle_request(&fixture.state, upsert_request("Note"))
        .await
        .expect("the previous executor writes an operation receipt");
    let snapshot_before_reopen = write_snapshot(&fixture.state).await;
    fixture.shutdown().await;

    let service = ArkService::open(fixture.db_path().to_str().unwrap())
        .await
        .expect("production actor reopens the database");
    service
        .call("upsert_object", upsert_params("Note"))
        .await
        .expect("the Store recognizes and replays the old receipt fingerprint");

    assert_eq!(
        service
            .call(
                "get_object_write_snapshot",
                json!({"id":"operation-receipt-note"})
            )
            .await
            .expect("object snapshot reads after reopen"),
        snapshot_before_reopen
    );
    assert_eq!(
        service
            .call("get_object", json!({"id":"operation-receipt-note"}))
            .await
            .expect("object reads after receipt replay")["title"],
        "Note"
    );
    service
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn clear_all_removes_operation_receipts_and_objects() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned(),)
    .await
    .unwrap();

    handle_request(&state, upsert_request("Private note payload"))
        .await
        .unwrap();
    handle_request(&state, Request::ClearAll).await.unwrap();
    handle_request(&state, upsert_request("Reused operation id"))
        .await
        .expect("ClearAll removes the old receipt so the operation id can be reused");
    assert_eq!(
        handle_request(
            &state,
            Request::GetObject {
                id: "operation-receipt-note".into(),
            },
        )
        .await
        .unwrap()["title"],
        "Reused operation id"
    );
}
