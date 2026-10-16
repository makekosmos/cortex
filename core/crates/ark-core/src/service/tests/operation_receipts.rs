use super::*;

fn upsert_request(title: &str) -> Request {
    Request::UpsertObject {
        object: ArkObjectWrite {
            id: "operation-receipt-note".into(),
            type_id: "com.kosmos.note".into(),
            type_version: Some("1.0.0".into()),
            title: title.into(),
            content_json: json!({"type":"doc","content":[]}),
            props_json: json!({"description":null,"extensions":{}}),
            created_at: "2026-10-16T00:00:00.000Z".into(),
            updated_at: "2026-10-16T00:00:00.000Z".into(),
            deleted_at: None,
        },
        expected_snapshot: None,
        operation_id: Some("json-operation-1".into()),
        device_id: Some("request-device-is-not-principal".into()),
    }
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
