use super::*;

#[tokio::test]
async fn usage_read_rpcs_return_empty_database_results() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().to_string())
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
            Request::UsageSummary {
                bindings: Vec::new(),
                range_start: None,
                range_end: None,
            },
        )
        .await
        .unwrap(),
        json!({"aggregates": [], "dailyTotals": [], "perBindingTotals": []})
    );
}

#[tokio::test]
async fn usage_summary_is_reached_by_its_wire_name() {
    let fixture = service_fixture();
    initialize_test_database(
        &fixture.state,
        fixture.db_path().to_string_lossy().into_owned(),
    )
    .await
    .unwrap();
    let service = ArkService::open(fixture.db_path().to_str().unwrap())
        .await
        .expect("production actor opens the database");

    let summary = service
        .call(
            "usage.summary",
            json!({"bindings": [], "range_start": null, "range_end": null}),
        )
        .await
        .expect("the dotted wire name matches the operation");
    assert_eq!(
        summary,
        json!({"aggregates": [], "dailyTotals": [], "perBindingTotals": []})
    );

    for stale in ["get_usage_summary", "usage_summary"] {
        let error = service
            .call(
                stale,
                json!({"bindings": [], "range_start": null, "range_end": null}),
            )
            .await
            .expect_err("only the declared wire name is accepted");
        assert!(
            format!("{error:?}").contains("unknown variant"),
            "{stale}: {error:?}"
        );
    }

    service
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
    fixture.shutdown().await;
}
