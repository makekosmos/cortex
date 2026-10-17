#[tokio::test]
async fn committed_feed_replays_same_id_after_lost_ack_and_rejects_digest_reuse() {
    let backend = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        setup_db(),
    ))));
    admit_test_registry_peer(&backend, "origin-peer").await;
    admit_test_registry_peer(&backend, "server-peer").await;
    let generation = backend.selective_sync_policy_generation();
    backend
        .begin_outbound_policy("server-peer", generation)
        .await
        .unwrap();
    let entity = full_core_task_entity(
        "feed-replay-object",
        "Replay me",
        "2026-10-16T00:00:00.000Z:000001:device-a",
    );
    backend
        .apply_authenticated_entity(&entity, "origin-peer")
        .await
        .unwrap();
    backend
        .complete_outbound_snapshot("server-peer", generation)
        .await
        .unwrap();

    let storage: Arc<dyn crate::replication_storage::SyncRuntimeStorage> = backend.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let flight: crate::sync_client::ClientFeedInFlight = Arc::new(tokio::sync::Mutex::new(None));
    crate::sync_client::pump_client_committed_feed(
        &storage,
        "server-peer",
        &tx,
        &flight,
        false,
        false,
    )
    .await
    .unwrap();
    let first_id = pop_feed_live_change_id(&mut rx);

    // Simulate disconnect after delivery but before the applied ACK arrives.
    *flight.lock().await = None;
    crate::sync_client::pump_client_committed_feed(
        &storage,
        "server-peer",
        &tx,
        &flight,
        false,
        false,
    )
    .await
    .unwrap();
    let replay_id = pop_feed_live_change_id(&mut rx);
    assert_eq!(first_id, replay_id, "lost ACK must replay the same ID");

    backend
        .apply_authenticated_entity_with_id(&entity, "server-peer", &replay_id)
        .await
        .unwrap();
    crate::sync_client::acknowledge_client_feed(
        &storage,
        "server-peer",
        &replay_id,
        &tx,
        &flight,
        ark_application::sync_storage::OutboundAck::Applied,
        false,
        false,
        None,
    )
    .await
    .unwrap();
    assert!(rx.try_recv().is_err(), "no later item should be in flight");

    let mut conflicting = entity;
    conflicting.data.insert("title".into(), json!("changed"));
    assert_eq!(
        backend
            .apply_authenticated_entity_with_id(&conflicting, "server-peer", &replay_id)
            .await
            .unwrap_err(),
        "sync_inbox_identity_digest_mismatch"
    );
}

#[tokio::test]
async fn legacy_feed_pump_omits_policy_identity_and_legacy_ack_advances_progress() {
    let backend = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        setup_db(),
    ))));
    admit_test_registry_peer(&backend, "legacy-peer").await;
    let digest = ark_application::sync_policy::digest_sync_policy(&[]);
    backend
        .accept_peer_sync_policy("legacy-peer", 1, &digest, false, &[])
        .await
        .unwrap();
    let generation = backend
        .selective_sync_policy_generation_for_peer("legacy-peer")
        .await
        .unwrap();
    backend
        .begin_outbound_policy("legacy-peer", generation)
        .await
        .unwrap();
    let entity = full_core_task_entity(
        "legacy-feed-object",
        "Legacy delivery",
        "2026-10-16T00:00:00.000Z:000001:device-a",
    );
    backend.apply_entity(&entity).await.unwrap();
    backend
        .complete_outbound_snapshot("legacy-peer", generation)
        .await
        .unwrap();

    let storage: Arc<dyn crate::replication_storage::SyncRuntimeStorage> = backend.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let flight: crate::sync_client::ClientFeedInFlight = Arc::new(tokio::sync::Mutex::new(None));
    crate::sync_client::pump_client_committed_feed(
        &storage,
        "legacy-peer",
        &tx,
        &flight,
        false,
        false,
    )
    .await
    .unwrap();
    let change_id = match rx.try_recv().unwrap() {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            match crate::protocol::deserialize_message(&text).unwrap() {
                crate::protocol::LanSyncMessage::LiveChange {
                    change_id,
                    policy_identity,
                    ..
                } => {
                    assert!(policy_identity.is_none());
                    change_id
                }
                message => panic!("unexpected feed message: {message:?}"),
            }
        }
        message => panic!("unexpected frame: {message:?}"),
    };
    let position = change_id
        .strip_prefix("ark-feed-v1:")
        .unwrap()
        .parse::<i64>()
        .unwrap();
    crate::sync_client::acknowledge_client_feed(
        &storage,
        "legacy-peer",
        &change_id,
        &tx,
        &flight,
        ark_application::sync_storage::OutboundAck::Applied,
        false,
        false,
        None,
    )
    .await
    .unwrap();
    let progress = backend
        .begin_outbound_policy("legacy-peer", generation)
        .await
        .unwrap();
    assert_eq!(progress.applied_through, position);
}
