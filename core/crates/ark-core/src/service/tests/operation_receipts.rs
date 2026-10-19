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

    assert_eq!(
        handle_request(&state, upsert_request("Note"))
            .await
            .unwrap(),
        json!(true)
    );
    assert_eq!(
        handle_request(&state, upsert_request("Note"))
            .await
            .unwrap(),
        json!(true)
    );
    assert!(handle_request(&state, upsert_request("Different payload"))
        .await
        .unwrap_err()
        .contains("different command payload"));

    with_conn(&state, |connection| {
        let receipts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_operation_receipts
                 WHERE principal = 'engine' AND operation_id = 'json-operation-1'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let revisions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM object_sync_versions
                 WHERE object_id = 'operation-receipt-note'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(receipts, 1);
        assert_eq!(revisions, 1);
        Ok(())
    })
    .unwrap();
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
    let revision_before = with_conn(&fixture.state, |connection| {
        connection
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id='operation-receipt-note'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();
    fixture.shutdown().await;

    let service = ArkService::open(fixture.db_path().to_str().unwrap())
        .await
        .expect("production actor reopens the database");
    service
        .call("upsert_object", upsert_params("Note"))
        .await
        .expect("the Store recognizes and replays the old receipt fingerprint");

    let connection = rusqlite::Connection::open(fixture.db_path()).unwrap();
    let (receipt_count, revision, title): (i64, String, String) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM ark_operation_receipts
                     WHERE principal='engine' AND operation_id='json-operation-1'),
                    object_sync_versions.hlc, objects.title
             FROM object_sync_versions JOIN objects ON objects.id=object_sync_versions.object_id
             WHERE object_sync_versions.object_id='operation-receipt-note'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(receipt_count, 1);
    assert_eq!(revision, revision_before);
    assert_eq!(title, "Note");
    service
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn clear_all_removes_operation_receipts_with_their_stored_results() {
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
    with_conn(&state, |connection| {
        let stored_result: String = connection
            .query_row(
                "SELECT result_json FROM ark_operation_receipts
                 WHERE principal = 'engine' AND operation_id = 'json-operation-1'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        assert!(stored_result.contains("Private note payload"));
        Ok(())
    })
    .unwrap();

    handle_request(&state, Request::ClearAll).await.unwrap();

    with_conn(&state, |connection| {
        let receipts: i64 = connection
            .query_row("SELECT COUNT(*) FROM ark_operation_receipts", [], |row| {
                row.get(0)
            })
            .map_err(|error| error.to_string())?;
        let objects: i64 = connection
            .query_row("SELECT COUNT(*) FROM objects", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        assert_eq!(receipts, 0);
        assert_eq!(objects, 0);
        Ok(())
    })
    .unwrap();
}
