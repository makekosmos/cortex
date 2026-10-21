#![allow(clippy::unwrap_used)]

use super::SqliteStorageBackend;
use ark_application::sync_storage::CommittedFeedPort;
use ark_store::test_support::TestDatabase;

#[tokio::test]
async fn sync_policy_generation_changes_when_only_filter_changes() {
    let database = TestDatabase::create().unwrap();
    let backend = SqliteStorageBackend::open_for_test(
        database.path().to_string_lossy().into_owned(),
        "test-device",
    )
    .await
    .unwrap();
    let mut profile = ark_domain::selective_sync::SelectiveSyncProfile::default();
    profile.set_rule(
        "type",
        "com.kosmos.note",
        ark_domain::selective_sync::SyncMode::Full,
    );
    backend.set_selective_sync_profile(Some(profile.clone()));
    let unfiltered = backend.selective_sync_policy_generation();
    profile.set_rule_with_filter(
        "type",
        "com.kosmos.note",
        ark_domain::selective_sync::SyncMode::Full,
        ark_domain::selective_sync::FilterV1::parse(
            r#"{"version":1,"where":{"eq":{"field":"title","value":"Note"}}}"#,
        )
        .unwrap(),
    );
    backend.set_selective_sync_profile(Some(profile));
    assert_ne!(unfiltered, backend.selective_sync_policy_generation());
    backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
