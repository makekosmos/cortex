fn full_core_task_entity(id: &str, title: &str, hlc: &str) -> SyncEntity {
    let mut entity = sync_object(id, "core.task", title);
    entity.hlc = hlc.into();
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );
    entity
}

#[tokio::test]
async fn authenticated_core_task_replay_is_idempotent_and_rejects_revision_reuse() {
    let backend = make_backend().await;
    admit_test_registry_peer(&backend, "device-a").await;
    admit_test_registry_peer(&backend, "relay-peer").await;
    let mut entity = sync_object("receipt-replay-task", "core.task", "Original title");
    entity.data.insert("typeVersion".into(), json!("0.1.0"));
    entity.data.insert(
        "propsJson".into(),
        json!({
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "inbox", "period": "any", "date": null},
            "deadline": null,
            "reminders": [],
            "recurrence": null,
            "priority": "none",
            "checklist": [],
            "extensions": {}
        }),
    );
    backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap();
    backend
        .apply_authenticated_entity(&entity, "relay-peer")
        .await
        .unwrap();
    let snapshot = backend.committed_feed_snapshot().await.unwrap();
    let page = backend
        .committed_feed_page(0, snapshot.high_watermark, 8)
        .await
        .unwrap();
    assert!(page.iter().any(|entry| entry.entity_id == entity.id));
    let policy = backend.begin_outbound_policy("device-a", 3).await.unwrap();
    assert_eq!(policy.scanned_through, 0);
    backend
        .complete_outbound_snapshot("device-a", 3)
        .await
        .unwrap();
    let scanned = backend
        .record_outbound_scan("device-a", 3, snapshot.high_watermark)
        .await
        .unwrap();
    assert_eq!(scanned.scanned_through, snapshot.high_watermark);
    let applied = backend
        .acknowledge_outbound(
            "device-a",
            3,
            snapshot.high_watermark,
            ark_application::sync_storage::OutboundAck::Applied,
        )
        .await
        .unwrap();
    assert_eq!(applied.applied_through, snapshot.high_watermark);
    let mut stale = entity.clone();
    stale.hlc = "2025-12-31T00:00:00.000Z:000001:device-a".into();
    stale
        .data
        .insert("propsJson".into(), json!("malformed stale payload"));
    backend
        .apply_authenticated_entity(&stale, "device-a")
        .await
        .unwrap();
    assert_eq!(
        backend
            .committed_feed_snapshot()
            .await
            .unwrap()
            .high_watermark,
        snapshot.high_watermark,
        "stale replay must not advance the committed feed"
    );
    entity.data.insert("title".into(), json!("Changed payload"));
    assert_eq!(
        backend
            .apply_authenticated_entity(&entity, "device-a")
            .await
            .unwrap_err(),
        "sync_inbox_identity_digest_mismatch"
    );
    let final_snapshot = backend.committed_feed_snapshot().await.unwrap();
    assert_eq!(final_snapshot.high_watermark, snapshot.high_watermark);
    let deliveries = expect_feed_deliveries(
        backend
            .committed_feed_delivery_page(0, final_snapshot.high_watermark, 8)
            .await
            .unwrap(),
    );
    let persisted = deliveries
        .iter()
        .find(|delivery| delivery.entry.entity_id == "receipt-replay-task")
        .expect("the original task remains in the committed feed");
    assert_eq!(
        persisted.entity.data.get("title"),
        Some(&json!("Original title")),
        "stale and mismatched replays must preserve the original payload"
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn feed_delivery_coalesces_entity_and_keeps_payload_revision_consistent_with_watermark() {
    let backend = make_backend().await;
    admit_test_registry_peer(&backend, "device-a").await;
    let entity = full_core_task_entity(
        "feed-projection",
        "Old payload",
        "2026-10-16T00:00:00.000Z:000001:device-a",
    );
    backend
        .apply_authenticated_entity(&entity, "device-a")
        .await
        .unwrap();
    let old_snapshot = backend.committed_feed_snapshot().await.unwrap();

    let mut newer = entity.clone();
    newer.hlc = "2026-10-16T00:00:01.000Z:000001:device-a".into();
    newer.data.insert("title".into(), json!("New payload"));
    backend
        .apply_authenticated_entity(&newer, "device-a")
        .await
        .unwrap();
    let latest_snapshot = backend.committed_feed_snapshot().await.unwrap();

    let latest = expect_feed_deliveries(
        backend
            .committed_feed_delivery_page(0, latest_snapshot.high_watermark, 8)
            .await
            .unwrap(),
    );
    let matches = latest
        .iter()
        .filter(|delivery| delivery.entry.entity_id == entity.id)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "same locator coalesces to one latest state"
    );
    assert_eq!(matches[0].entry.revision, newer.hlc);
    assert_eq!(matches[0].entity.hlc, newer.hlc);
    assert_eq!(
        matches[0].entity.data.get("title"),
        Some(&json!("New payload"))
    );

    let pinned_old = expect_feed_deliveries(
        backend
            .committed_feed_delivery_page(0, old_snapshot.high_watermark, 8)
            .await
            .unwrap(),
    );
    assert!(
        pinned_old
            .iter()
            .all(|delivery| delivery.entry.entity_id != entity.id),
        "a post-watermark payload must not be paired with an older cursor"
    );
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn authenticated_core_note_update_is_durable_and_relayable() {
    let source_test = make_backend().await;
    let relay_test = make_backend().await;
    let _source_database = source_test._database;
    let _relay_database = relay_test._database;
    let source = Arc::new(source_test.backend);
    let relay = Arc::new(relay_test.backend);
    admit_test_registry_peer(&source, "origin-peer").await;
    admit_test_registry_peer(&source, "another-peer").await;
    admit_test_registry_peer(&relay, "source-peer").await;
    let snapshot_row = sync_object("old-feed-row", "core.note", "Covered by initial snapshot");
    source.apply_entity(&snapshot_row).await.unwrap();
    let generation = source.selective_sync_policy_generation();
    let baseline = source
        .begin_outbound_policy("relay-peer", generation)
        .await
        .unwrap()
        .initial_snapshot
        .through();
    let mut note = sync_object("core-note-feed-relay", "core.note", "First title");
    source
        .apply_authenticated_entity(&note, "origin-peer")
        .await
        .unwrap();
    note.hlc = "2026-10-16T00:00:01.000Z:000001:origin-peer".into();
    note.data
        .insert("title".into(), json!("Live updated title"));
    source
        .apply_authenticated_entity(&note, "origin-peer")
        .await
        .unwrap();
    let mut stale = note.clone();
    stale.hlc = "2026-10-15T23:59:59.000Z:000001:origin-peer".into();
    stale.data.insert("title".into(), json!("Stale title"));
    source
        .apply_authenticated_entity(&stale, "another-peer")
        .await
        .unwrap();

    let snapshot = source.committed_feed_snapshot().await.unwrap();
    let page = expect_feed_deliveries(
        source
            .committed_feed_delivery_page(0, snapshot.high_watermark, 8)
            .await
            .unwrap(),
    );
    let delivery = page
        .iter()
        .find(|delivery| delivery.entry.entity_id == note.id)
        .expect("accepted core.note update must append a feed entry");
    assert_eq!(delivery.entry.entity_type, "object");
    assert_eq!(delivery.entry.revision, note.hlc);
    assert_eq!(
        delivery.entity.data.get("title"),
        Some(&json!("Live updated title"))
    );
    let raw_page = source
        .committed_feed_page(0, snapshot.high_watermark, 8)
        .await
        .unwrap();
    assert_eq!(
        raw_page
            .iter()
            .filter(|entry| entry.entity_id == note.id)
            .count(),
        2,
        "stale apply must not append another feed position"
    );

    let source_storage: Arc<dyn crate::replication_storage::SyncRuntimeStorage> = source.clone();
    let (tx, mut rx) = ark_sync::ClientMessageSender::test_channel();
    let flight: ark_sync::ClientFeedInFlight = Arc::new(tokio::sync::Mutex::new(None));
    ark_sync::pump_client_committed_feed(&source_storage, "relay-peer", &tx, &flight, false, false)
        .await
        .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "feed must wait for the initial snapshot ACK"
    );
    source
        .complete_outbound_snapshot("relay-peer", generation)
        .await
        .unwrap();
    ark_sync::pump_client_committed_feed(&source_storage, "relay-peer", &tx, &flight, false, false)
        .await
        .unwrap();
    let change_id = pop_feed_live_change_id(&mut rx);
    assert!(
        change_id
            .strip_prefix("ark-feed-v1:")
            .unwrap()
            .parse::<i64>()
            .unwrap()
            > baseline,
        "initial snapshot feed rows must not drain as live updates"
    );
    relay
        .apply_authenticated_entity_with_id(&delivery.entity, "source-peer", &change_id)
        .await
        .unwrap();

    let relay_snapshot = relay.committed_feed_snapshot().await.unwrap();
    let relayed = expect_feed_deliveries(
        relay
            .committed_feed_delivery_page(0, relay_snapshot.high_watermark, 8)
            .await
            .unwrap(),
    );
    assert!(
        relayed.iter().any(|item| {
            item.entry.entity_type == "object"
                && item.entry.entity_id == note.id
                && item.entity.data.get("title") == Some(&json!("Live updated title"))
        }),
        "inbound core.note update must enter the relay's durable outbound feed"
    );
    source
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
    relay
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn object_tombstone_feed_projection_preserves_type_id_for_selective_sync() {
    let backend = make_backend().await;
    admit_test_registry_peer(&backend, "origin-peer").await;
    let object = full_core_task_entity(
        "typed-feed-tombstone",
        "Delete me",
        "2026-10-16T00:00:00.000Z:000001:device-a",
    );
    backend
        .apply_authenticated_entity(&object, "origin-peer")
        .await
        .unwrap();
    let mut tombstone = object;
    tombstone.hlc = "2026-10-16T00:00:01.000Z:000001:device-a".into();
    tombstone.deleted = Some(true);
    backend
        .apply_authenticated_entity(&tombstone, "origin-peer")
        .await
        .unwrap();

    let snapshot = backend.committed_feed_snapshot().await.unwrap();
    let deliveries = expect_feed_deliveries(
        backend
            .committed_feed_delivery_page(0, snapshot.high_watermark, 8)
            .await
            .unwrap(),
    );
    let deleted = deliveries
        .iter()
        .find(|delivery| delivery.entry.entity_id == tombstone.id && delivery.entry.deleted)
        .expect("tombstone is present in committed feed");
    assert_eq!(deleted.entity.data.get("typeId"), Some(&json!("core.task")));
    backend
        .backend
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
