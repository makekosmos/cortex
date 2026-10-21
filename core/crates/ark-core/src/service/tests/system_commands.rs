use super::*;

#[tokio::test]
async fn set_sync_kv_request_uses_interactive_db_command() {
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

    let request: Request = serde_json::from_value(json!({
        "operation": "set_sync_kv",
        "key": "test.local-setting",
        "value": "enabled"
    }))
    .unwrap();
    assert_eq!(handle_request(&state, request).await.unwrap(), json!(true));

    let stored = handle_request(
        &state,
        Request::GetSyncKv {
            key: "test.local-setting".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(stored, json!("enabled"));
}
