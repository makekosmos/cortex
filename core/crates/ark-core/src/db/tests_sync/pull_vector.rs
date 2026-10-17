    /// KOS-370 regression: a pull-side vector snapshot is stale by the time
    /// `is_last` persists it — the incoming session loads it at the first
    /// batch and keeps it in memory for the whole exchange. Keys written
    /// concurrently (local mutations, integration-replication bumps like
    /// `integration_credential_envelope:*`) must survive the save.
    #[tokio::test]
    async fn persist_pull_vector_preserves_concurrent_keys() {
        let shared = Arc::new(Mutex::new(setup_db()));
        let backend: Arc<dyn crate::sync_server::SyncRuntimeStorage> =
            Arc::new(SqliteStorageBackend::for_test(shared.clone()));

        let mut pull_vector = VersionVector::new();
        for id in ["entity-a", "entity-b"] {
            pull_vector.insert(
                id.to_string(),
                "2026-01-01T00:00:00.000Z:000000:peer".to_string(),
            );
        }

        {
            let guard = shared.lock().unwrap_or_else(|e| e.into_inner());
            let mut stored = stored_vector(&guard);
            for (key, hlc) in [
                (
                    "integration_credential_envelope:integration-a:node-b:1",
                    "2026-01-03T00:00:00.000Z:000000:local",
                ),
                ("entity-b", "2026-01-02T00:00:00.000Z:000000:local"),
            ] {
                stored.insert(key.to_string(), hlc.to_string());
            }
            save_stored_vector(&guard, &stored);
        }
        ark_sync::inbound_snapshot::persist_pull_vector(
            backend.as_ref(),
            &mut pull_vector,
            true,
            true,
            None,
        )
        .await
        .unwrap();
        let guard = shared.lock().unwrap_or_else(|e| e.into_inner());
        let stored = stored_vector(&guard);
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
