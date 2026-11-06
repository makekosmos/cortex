use kernel::service::ArkService;
use serde_json::json;
use std::path::Path;
use std::time::{Duration, Instant};

fn copy_fixture() -> (tempfile::TempDir, String) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/old-db/ark.db");
    let directory = tempfile::tempdir().expect("old-db fixture temp dir");
    let path = directory.path().join("ark.db");
    std::fs::copy(&fixture, &path).expect("copy old-db fixture");
    let path = path.to_string_lossy().into_owned();
    (directory, path)
}

#[tokio::test]
async fn old_database_with_pomodoro_and_integration_rows_still_opens_and_reads() {
    let (_directory, path) = copy_fixture();

    let connection = store::open::open_db(&path).expect("old-db opens with sqlite");
    for table in [
        "integration_configurations",
        "authorized_nodes",
        "integration_node_grants",
        "integration_credential_envelopes",
        "integration_refresh_leases",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("integration table remains readable");
        assert_eq!(count, 1, "{table} row must survive the upgrade");
    }
    drop(connection);

    let service = ArkService::open(path)
        .await
        .expect("old database opens with the current service");
    let entries = service
        .call("objects.list", json!({"typeId": "core.time-entry"}))
        .await
        .expect("old time entries list");
    assert_eq!(entries.as_array().map(Vec::len), Some(3));

    for id in ["old-pomodoro-1", "old-pomodoro-2"] {
        let object = service
            .call("objects.get", json!({"id": id}))
            .await
            .expect("old pomodoro entry reads");
        assert_eq!(object["propsJson"]["source"], "pomodoro");
    }
    let manual = service
        .call("objects.get", json!({"id": "old-manual-1"}))
        .await
        .expect("old manual entry reads");
    assert_eq!(manual["propsJson"]["source"], "manual");

    service
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("service shuts down");
}
