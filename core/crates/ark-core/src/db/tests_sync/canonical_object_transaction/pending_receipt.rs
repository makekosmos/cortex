use ark_application::sync_storage::PeerProgressPort;

#[tokio::test]
async fn pending_authenticated_object_is_accepted() {
    let backend = make_backend().await;
    admit_test_registry_peer(&backend, "peer-a").await;
    let mut entity = sync_object("pending-receipt", "unknown-type", "Future object");
    entity.data.insert("typeVersion".into(), json!("9.9.9"));

    backend
        .apply_authenticated_entity_with_id(&entity, "peer-a", "feed-change-1")
        .await
        .unwrap();
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

fn save_test_local_policy(
    path: &std::path::Path,
    expected_revision: Option<i64>,
) -> ark_application::sync_policy::SyncProfile {
    ark_store::test_support::SyncTestState::save_local_profile(path, expected_revision).unwrap()
}

fn test_policy_identity(
    profile: &ark_application::sync_policy::SyncProfile,
) -> ark_domain::sync_policy::PolicyIdentity {
    ark_sync::protocol::sync_policy_identity(Some(profile))
}

#[tokio::test]
async fn stale_policy_at_receipt_transaction_rejects_before_inbox_write() {
    let backend = make_backend().await;
    admit_test_registry_peer(&backend, "peer-a").await;
    let old_policy = test_policy_identity(&save_test_local_policy(backend.database().path(), None));
    let current_policy =
        test_policy_identity(&save_test_local_policy(backend.database().path(), Some(1)));
    let entity = sync_object("stale-receipt", "core.note", "Must not persist");

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
    let mut retry = entity.clone();
    retry
        .data
        .insert("title".into(), json!("Retry with a new payload"));
    assert!(
        backend
            .receive_authenticated_entity_with_policy(
                &retry,
                "peer-a",
                "stale-receipt-message",
                "device-local",
                Some(current_policy),
            )
            .await
            .unwrap(),
        "stale policy rejection must not reserve the message id for the old payload"
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn stale_policy_after_receipt_rejects_apply_without_advancing_inbox_or_objects() {
    let backend = make_backend().await;
    admit_test_registry_peer(&backend, "peer-a").await;
    let old_policy = test_policy_identity(&save_test_local_policy(backend.database().path(), None));
    let entity = sync_object("stale-apply", "core.note", "Must remain unapplied");
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
    let current_policy =
        test_policy_identity(&save_test_local_policy(backend.database().path(), Some(1)));

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
    assert_eq!(
        backend
            .committed_feed_snapshot()
            .await
            .unwrap()
            .high_watermark,
        0,
        "stale policy rejection must not apply the received entity"
    );
    let stale_vector = crate::types::VersionVector::from([(entity.id.clone(), entity.hlc.clone())]);
    let vector_error = backend
        .merge_version_vector_with_policy("device-local", &old_policy, &stale_vector)
        .await
        .unwrap_err();
    assert!(vector_error.contains("stale_or_unnegotiated_sync_policy"));
    assert!(backend.load_version_vector().await.unwrap().is_empty());
    backend
        .apply_authenticated_entity_with_policy(
            &entity,
            "peer-a",
            "stale-apply-message",
            "device-local",
            Some(current_policy),
        )
        .await
        .expect("the received entity remains replayable under the current policy");
    let replayed = backend.committed_feed_snapshot().await.unwrap();
    assert!(replayed.high_watermark > 0);
    assert!(expect_feed_deliveries(
        backend
            .committed_feed_delivery_page(0, replayed.high_watermark, 8)
            .await
            .unwrap()
    )
    .iter()
    .any(|entry| entry.entry.entity_id == entity.id));
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
