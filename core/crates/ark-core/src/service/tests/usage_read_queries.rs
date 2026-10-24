use super::*;

#[tokio::test]
async fn usage_read_rpcs_return_empty_database_results() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().to_string(),)
    .await
    .unwrap();

    assert_eq!(
        handle_request(
            &state,
            Request::GetUsageTitleTotal {
                query: "focus".into(),
            },
        )
        .await
        .unwrap(),
        json!({"activeSeconds": 0, "idleSeconds": 0})
    );
    assert_eq!(
        handle_request(&state, Request::ListRecentUsageProcesses { limit: None })
            .await
            .unwrap(),
        json!([])
    );
    assert_eq!(
        handle_request(
            &state,
            Request::SearchUsageProcesses {
                query: "editor".into(),
                limit: None,
            },
        )
        .await
        .unwrap(),
        json!([])
    );

    let analytics = handle_request(
        &state,
        Request::GetUsageAnalytics {
            range_days: None,
            top_apps_limit: None,
            recent_sessions_limit: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(analytics["summary"]["trackedAppCount"], json!(0));
    assert_eq!(analytics["summary"]["sessionCount"], json!(0));
    assert_eq!(analytics["topApps"], json!([]));

    assert_eq!(
        handle_request(
            &state,
            Request::GetUsageGamePlaytimeSummary {
                bindings: Vec::new(),
                range_start: None,
                range_end: None,
            },
        )
        .await
        .unwrap(),
        json!({"aggregates": [], "dailyTotals": [], "perGameTotals": []})
    );
}
