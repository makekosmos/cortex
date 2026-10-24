use super::*;

#[tokio::test]
async fn usage_span_command_commits_days_and_rolls_back_invalid_timestamp() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().to_string())
        .await
        .unwrap();

    let response = handle_request(
        &state,
        Request::UpsertUsageSpan {
            usage_span: UsageSpanWrite {
                device_id: "usage-command-device".to_string(),
                started_at_unix: 1_777_939_199,
                ended_at_unix: 1_777_939_201,
                tracked_app_id: "usage-command-app".to_string(),
                window_title: Some("crosses midnight".to_string()),
                flags: 0,
                updated_at: "2026-05-01T00:00:00.000Z".to_string(),
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        response,
        json!({ "usageDayIds": [
        "usage-day:usage-command-device:2026-05-04",
        "usage-day:usage-command-device:2026-05-05"
    ] })
    );

    let error = handle_request(
        &state,
        Request::UpsertUsageSpan {
            usage_span: UsageSpanWrite {
                device_id: "usage-command-invalid-device".to_string(),
                started_at_unix: i64::MAX - 1,
                ended_at_unix: i64::MAX,
                tracked_app_id: "usage-command-app".to_string(),
                window_title: None,
                flags: 0,
                updated_at: "2026-05-01T00:00:00.000Z".to_string(),
            },
        },
    )
    .await
    .unwrap_err();
    assert!(error.contains("invalid usage span timestamp"), "{error}");

    let persisted = ark_store::test_support::UsageRollbackFixture::inspect(
        &fixture.db_path(),
        "usage-command-device",
    )
    .unwrap();
    assert_eq!(persisted.usage_days, 2);

    let invalid_device = ark_store::test_support::UsageRollbackFixture::inspect(
        &fixture.db_path(),
        "usage-command-invalid-device",
    )
    .unwrap();
    assert_eq!(invalid_device.usage_days, 0);
    assert_eq!(invalid_device.sync_heads, 0);
    assert_eq!(invalid_device.sync_log, 0);
    assert_eq!(invalid_device.sync_versions, 0);
}
