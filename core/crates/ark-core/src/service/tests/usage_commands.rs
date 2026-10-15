use super::*;

#[tokio::test]
async fn usage_span_command_commits_days_and_rolls_back_invalid_timestamp() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().to_string(),
        },
    )
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

    let connection = get_db_handle(&state)
        .map(|db| db.shared_connection_for_test())
        .unwrap();
    let guard = connection.lock().unwrap();
    let usage_days: i64 = guard
        .query_row("SELECT COUNT(*) FROM usage_days", [], |row| row.get(0))
        .unwrap();
    assert_eq!(usage_days, 2);
    let failed_device_head: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM usage_sync_heads WHERE device_id = ?1",
            ["usage-command-invalid-device"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failed_device_head, 0);
    let failed_device_log: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM usage_sync_log WHERE device_id = ?1",
            ["usage-command-invalid-device"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failed_device_log, 0);
    let failed_device_versions: i64 = guard
        .query_row(
            "SELECT COUNT(*) FROM usage_sync_versions WHERE entity_id LIKE ?1",
            ["usage-day:usage-command-invalid-device:%"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failed_device_versions, 0);
}
