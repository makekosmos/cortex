use ark_application::sync_storage::PeerProgressPort;

#[tokio::test]
async fn pending_authenticated_object_keeps_a_durable_receipt_without_applied_state() {
    let backend = SqliteStorageBackend::for_test(Arc::new(Mutex::new(setup_db())));
    admit_test_registry_peer(&backend, "peer-a").await;
    let mut entity = sync_object("pending-receipt", "unknown-type", "Future object");
    entity.data.insert("typeVersion".into(), json!("9.9.9"));

    backend
        .apply_authenticated_entity_with_id(&entity, "peer-a", "feed-change-1")
        .await
        .unwrap();

    let shared = backend.db.shared_connection_for_test();
    let conn = shared.lock().unwrap();
    let (state, payload_bytes): (String, i64) = conn
        .query_row(
            "SELECT state, payload_bytes FROM ark_sync_inbox
             WHERE peer_device_id = 'peer-a' AND message_id = 'feed-change-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, "pending");
    assert!(payload_bytes > 0, "pending payload must remain durable");
    assert!(get_object(&conn, &entity.id).unwrap().is_none());
}

fn save_test_local_policy(
    conn: &rusqlite::Connection,
    expected_revision: Option<i64>,
) -> ark_application::sync_policy::SyncProfile {
    ark_store::data_platform::upsert_sync_profile(
        conn,
        "device-local",
        "local-policy",
        "Local policy",
        true,
        expected_revision,
        &[],
    )
    .unwrap()
}

fn test_policy_identity(
    profile: &ark_application::sync_policy::SyncProfile,
) -> ark_application::sync_storage::PolicyIdentity {
    crate::protocol::sync_policy_identity(Some(profile))
}

#[tokio::test]
async fn stale_policy_at_receipt_transaction_rejects_before_inbox_write() {
    let shared = Arc::new(Mutex::new(setup_db()));
    let backend = SqliteStorageBackend::for_test(shared.clone());
    admit_test_registry_peer(&backend, "peer-a").await;
    let old_policy = {
        let conn = shared.lock().unwrap();
        test_policy_identity(&save_test_local_policy(&conn, None))
    };
    {
        let conn = shared.lock().unwrap();
        save_test_local_policy(&conn, Some(1));
    }
    let entity = sync_object("stale-receipt", "note_obj", "Must not persist");

    let error = backend
        .receive_authenticated_entity_with_policy(
            &entity,
            "peer-a",
            "stale-receipt-message",
            "device-local",
            Some(old_policy.clone()),
        )
        .await
        .unwrap_err();
    assert!(error.contains("stale_or_unnegotiated_sync_policy"));
    let conn = shared.lock().unwrap();
    let receipt_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ark_sync_inbox WHERE message_id='stale-receipt-message'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(receipt_count, 0);
    assert!(get_object(&conn, &entity.id).unwrap().is_none());
}

#[tokio::test]
async fn stale_policy_after_receipt_rejects_apply_without_advancing_inbox_or_objects() {
    let shared = Arc::new(Mutex::new(setup_db()));
    let backend = SqliteStorageBackend::for_test(shared.clone());
    admit_test_registry_peer(&backend, "peer-a").await;
    let (old_policy, entity) = {
        let conn = shared.lock().unwrap();
        let policy = test_policy_identity(&save_test_local_policy(&conn, None));
        (
            policy,
            sync_object("stale-apply", "note_obj", "Must remain unapplied"),
        )
    };
    assert!(backend
        .receive_authenticated_entity_with_policy(
            &entity,
            "peer-a",
            "stale-apply-message",
            "device-local",
            Some(old_policy.clone()),
        )
        .await
        .unwrap());
    {
        let conn = shared.lock().unwrap();
        save_test_local_policy(&conn, Some(1));
    }

    let error = backend
        .apply_authenticated_entity_with_policy(
            &entity,
            "peer-a",
            "stale-apply-message",
            "device-local",
            Some(old_policy.clone()),
        )
        .await
        .unwrap_err();
    assert!(error.contains("stale_or_unnegotiated_sync_policy"));
    {
        let conn = shared.lock().unwrap();
        let state: String = conn
            .query_row(
                "SELECT state FROM ark_sync_inbox WHERE message_id='stale-apply-message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "received");
        assert!(get_object(&conn, &entity.id).unwrap().is_none());
    }
    let stale_vector = crate::types::VersionVector::from([(entity.id.clone(), entity.hlc.clone())]);
    let vector_error = backend
        .store_version_vector_with_policy("device-local", &old_policy, &stale_vector)
        .await
        .unwrap_err();
    assert!(vector_error.contains("stale_or_unnegotiated_sync_policy"));
    assert!(backend.load_version_vector().await.unwrap().is_empty());
}
