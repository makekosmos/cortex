/// KOS-370 regression: a pull-side vector snapshot is stale by the time
/// `is_last` persists it — the incoming session loads it at the first
/// batch and keeps it in memory for the whole exchange. Keys written
/// concurrently (local mutations, integration-replication bumps like
/// `integration_credential_envelope:*`) must survive the save.
#[tokio::test]
async fn persist_pull_vector_preserves_concurrent_keys() {
    let test_backend = make_backend().await;
    let backend: &dyn crate::sync_server::SyncRuntimeStorage = &*test_backend;

    let mut pull_vector = VersionVector::new();
    for id in ["entity-a", "entity-b"] {
        pull_vector.insert(
            id.to_string(),
            "2026-01-01T00:00:00.000Z:000000:peer".to_string(),
        );
    }

    let mut stored_vector = pull_vector.clone();
    for (key, hlc) in [
        (
            "integration_credential_envelope:integration-a:node-b:1",
            "2026-01-03T00:00:00.000Z:000000:local",
        ),
        ("entity-b", "2026-01-02T00:00:00.000Z:000000:local"),
    ] {
        stored_vector.insert(key.to_string(), hlc.to_string());
    }
    ark_store::test_support::SyncTestState::save_version_vector(
        test_backend.database().path(),
        &stored_vector,
    )
    .unwrap();
    ark_sync::inbound_snapshot::persist_pull_vector(backend, &mut pull_vector, true, true, None)
        .await
        .unwrap();
    let stored =
        ark_store::test_support::SyncTestState::load_version_vector(test_backend.database().path())
            .unwrap();
    let get = |key: &str| stored.get(key).map(String::as_str);
    assert_eq!(
        get("integration_credential_envelope:integration-a:node-b:1"),
        Some("2026-01-03T00:00:00.000Z:000000:local"),
    );
    assert_eq!(
        get("entity-a"),
        Some("2026-01-01T00:00:00.000Z:000000:peer"),
    );
    assert_eq!(
        get("entity-b"),
        Some("2026-01-02T00:00:00.000Z:000000:local"),
    );
}
