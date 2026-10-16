#[tokio::test]
async fn committed_feed_replays_same_id_after_lost_ack_and_rejects_digest_reuse() {
    let backend = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        setup_db(),
    ))));
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
    crate::sync_client::pump_client_committed_feed(&storage, "server-peer", &tx, &flight, false)
        .await
        .unwrap();
    let first_id = pop_feed_live_change_id(&mut rx);

    // Simulate disconnect after delivery but before the applied ACK arrives.
    *flight.lock().await = None;
    crate::sync_client::pump_client_committed_feed(&storage, "server-peer", &tx, &flight, false)
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
        crate::application::sync_storage::OutboundAck::Applied,
        false,
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
