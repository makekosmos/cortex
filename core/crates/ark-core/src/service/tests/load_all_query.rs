use super::*;

#[tokio::test]
async fn load_all_rpc_returns_the_legacy_export_shape() {
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

    let data = handle_request(&state, Request::LoadAll).await.unwrap();
    for collection in [
        "todos",
        "projects",
        "areas",
        "tags",
        "headings",
        "trackedApps",
        "usageSessions",
        "usageEvents",
        "objects",
        "objectTypes",
        "objectTypeSummaries",
        "objectTypeVersions",
        "objectTypeAliases",
        "objectLinks",
    ] {
        assert!(data[collection].is_array(), "missing {collection}: {data}");
    }
    assert_eq!(data["todos"], json!([]));
    assert_eq!(data["projects"], json!([]));
    assert_eq!(data["objects"], json!([]));
}
