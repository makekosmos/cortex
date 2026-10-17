use super::*;

#[tokio::test]
async fn estimate_data_volume_preserves_request_and_result_contract_through_actor() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("service database should open");

    with_write_tx(&state, |connection| {
        connection
            .execute(
                "INSERT INTO object_types
                 (id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked)
                 VALUES ('estimate_type','Estimate','{}','{}',
                 '2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z',0)",
                [],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO objects (id,type_id,type_version,title,content_json,props_json,
                 created_at,updated_at,deleted_at)
                 VALUES ('estimate-1','estimate_type','1.0.0','Visible title',
                 '{\"text\":\"private body\"}', '{\"allowed\":\"yes\"}',
                 '2026-01-01T00:00:00.000Z','2026-01-02T00:00:00.000Z',NULL)",
                [],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .expect("fixture objects should be inserted");

    let request: Request = serde_json::from_value(json!({
        "operation": "estimate_data_volume",
        "scopes": [{
            "typeId": "estimate_type",
            "typeVersion": "1.0.0",
            "fields": ["title", "props.allowed"]
        }]
    }))
    .expect("data volume request should decode");
    let result = handle_request(&state, request)
        .await
        .expect("data volume query should succeed");
    assert_eq!(
        result,
        json!([{
            "type_id": "estimate_type",
            "type_version": "1.0.0",
            "object_count": 1,
            "estimated_json_bytes": 176
        }])
    );
    let serialized = result.to_string();
    assert!(!serialized.contains("Visible title"));
    assert!(!serialized.contains("private body"));
}

#[test]
fn estimate_data_volume_rejects_unknown_scope_fields() {
    let error = serde_json::from_value::<Request>(json!({
        "operation": "estimate_data_volume",
        "scopes": [{
            "typeId": "estimate_type",
            "typeVersion": "1.0.0",
            "fields": ["title"],
            "includeAll": true
        }]
    }))
    .expect_err("unknown scope properties should be rejected")
    .to_string();
    assert!(error.contains("unknown field `includeAll`"), "{error}");
}
