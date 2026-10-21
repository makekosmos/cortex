#[tokio::test]
async fn negotiated_receipt_ack_advances_delivery_without_claiming_application() {
    let test_backend = make_backend().await;
    let _database = test_backend._database;
    let backend = Arc::new(test_backend.backend);
    admit_test_registry_peer(&backend, "origin-peer").await;
    let generation = backend.selective_sync_policy_generation();
    backend
        .begin_outbound_policy("receipt-peer", generation)
        .await
        .unwrap();
    for (id, title, hlc) in [
        (
            "receipt-one",
            "First",
            "2026-10-16T00:00:00.000Z:000001:device-a",
        ),
        (
            "receipt-two",
            "Second",
            "2026-10-16T00:00:00.001Z:000001:device-a",
        ),
    ] {
        let entity = full_core_task_entity(id, title, hlc);
        backend
            .apply_authenticated_entity(&entity, "origin-peer")
            .await
            .unwrap();
    }
    backend
        .complete_outbound_snapshot("receipt-peer", generation)
        .await
        .unwrap();

    let storage: Arc<dyn crate::replication_storage::SyncRuntimeStorage> = backend.clone();
    let (tx, mut rx) = ark_sync::ClientMessageSender::test_channel();
    let flight: ark_sync::ClientFeedInFlight = Arc::new(tokio::sync::Mutex::new(None));
    ark_sync::pump_client_committed_feed(&storage, "receipt-peer", &tx, &flight, true, false)
        .await
        .unwrap();
    let first_id = pop_feed_live_change_id(&mut rx);
    let first_position = first_id
        .strip_prefix("ark-feed-v1:")
        .unwrap()
        .parse::<i64>()
        .unwrap();

    ark_sync::acknowledge_client_feed(
        &storage,
        "receipt-peer",
        &first_id,
        &tx,
        &flight,
        ark_application::sync_storage::OutboundAck::DurableReceipt,
        true,
        false,
        None,
    )
    .await
    .unwrap();
    let after_first_receipt = backend
        .begin_outbound_policy("receipt-peer", generation)
        .await
        .unwrap();
    assert_eq!(after_first_receipt.receipt_through, first_position);
    assert_eq!(after_first_receipt.applied_through, 0);
    let second_id = pop_feed_live_change_id(&mut rx);
    assert_ne!(first_id, second_id);

    ark_sync::acknowledge_client_feed(
        &storage,
        "receipt-peer",
        &second_id,
        &tx,
        &flight,
        ark_application::sync_storage::OutboundAck::DurableReceipt,
        true,
        false,
        None,
    )
    .await
    .unwrap();
    let after_second_receipt = backend
        .begin_outbound_policy("receipt-peer", generation)
        .await
        .unwrap();
    assert_eq!(after_second_receipt.applied_through, 0);
    assert!(rx.try_recv().is_err());
}
