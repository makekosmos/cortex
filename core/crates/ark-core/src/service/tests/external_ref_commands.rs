use super::*;

#[tokio::test]
async fn external_ref_request_keeps_wire_shape_and_rolls_back_failed_update() {
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

    with_conn(&state, |connection| {
        let stored: (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
        ) = connection
            .query_row(
                "SELECT connector_id, account_id, external_type, external_id, object_id, \
                 external_revision, content_hash, conflict_state FROM external_refs",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(
            stored,
            (
                "drive".into(),
                "personal".into(),
                "file".into(),
                "file-1".into(),
                "external-ref-object".into(),
                Some("r1".into()),
                Some("abc".into()),
                "clean".into(),
            )
        );
        Ok(())
    })
    .unwrap();

    with_write_tx(&state, |connection| {
        connection
            .execute_batch(
                "CREATE TABLE tx_parent (id INTEGER PRIMARY KEY); \
                 CREATE TABLE tx_child ( \
                     parent_id INTEGER REFERENCES tx_parent(id) DEFERRABLE INITIALLY DEFERRED \
                 ); \
                 CREATE TRIGGER reject_external_ref_commit AFTER UPDATE ON external_refs \
                 BEGIN INSERT INTO tx_child(parent_id) VALUES (404); END;",
            )
            .map_err(|error| error.to_string())
    })
    .unwrap();

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

    with_conn(&state, |connection| {
        assert!(connection.is_autocommit());
        let stored: (String, Option<String>) = connection
            .query_row(
                "SELECT external_revision, content_hash FROM external_refs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(stored, ("r1".into(), Some("abc".into())));
        Ok(())
    })
    .unwrap();
}
