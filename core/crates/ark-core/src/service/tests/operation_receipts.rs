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

fn operation_receipt_count(path: &std::path::Path, operation_id: &str) -> i64 {
    rusqlite::Connection::open(path)
        .expect("database opens for receipt inspection")
        .query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )
        .expect("receipt count reads")
}

fn object_write_count(path: &std::path::Path) -> i64 {
    rusqlite::Connection::open(path)
        .expect("database opens for mutation inspection")
        .query_row("SELECT writes FROM operation_mutation_probe", [], |row| {
            row.get(0)
        })
        .expect("mutation count reads")
}

#[tokio::test]
async fn object_mutation_json_replays_same_result_and_rejects_operation_id_reuse() {
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
    rusqlite::Connection::open(fixture.db_path())
        .expect("database opens for mutation probe")
        .execute_batch(
            "CREATE TABLE operation_mutation_probe(writes INTEGER NOT NULL);
             INSERT INTO operation_mutation_probe VALUES (0);
             CREATE TRIGGER count_object_insert AFTER INSERT ON objects
             BEGIN UPDATE operation_mutation_probe SET writes = writes + 1; END;
             CREATE TRIGGER count_object_update AFTER UPDATE ON objects
             BEGIN UPDATE operation_mutation_probe SET writes = writes + 1; END;",
        )
        .expect("mutation counter triggers install");

    let (left, right) = tokio::join!(
        handle_request(&state, upsert_request("Note")),
        handle_request(&state, upsert_request("Note")),
    );
    assert_eq!(left.unwrap(), json!(true));
    assert_eq!(right.unwrap(), json!(true));
    assert_eq!(
        operation_receipt_count(&fixture.db_path(), "json-operation-1"),
        1,
        "concurrent duplicates commit only one receipt"
    );
    assert_eq!(
        object_write_count(&fixture.db_path()),
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
        operation_receipt_count(&fixture.db_path(), "json-operation-1"),
        1,
        "a conflicting payload does not create another receipt"
    );
    assert_eq!(
        object_write_count(&fixture.db_path()),
        1,
        "a conflicting payload does not execute another object mutation"
    );
}

#[tokio::test]
async fn production_store_replays_receipt_written_by_the_previous_core_executor() {
    let fixture = service_fixture();
    handle_request(
        &fixture.state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
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
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
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
