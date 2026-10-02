#![allow(clippy::unwrap_used)]

include!("support/iroh_bidirectional_support.rs");
#[tokio::test(flavor = "multi_thread")]
async fn iroh_bidirectional_hello_and_reverse_send() {
    let (b_events_tx, mut b_events_rx) = mpsc::unbounded_channel::<TransportEvent>();
    let transport_b = Arc::new(IrohTransport::new(IrohConfig {
        device_id: "device-B".to_string(),
        device_name: "Device B".to_string(),
        space_id: "test-space-iroh-bidirectional".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    }));
    transport_b
        .start(b_events_tx)
        .await
        .expect("transport B start");
    let ticket_b = transport_b
        .our_ticket()
        .await
        .expect("transport B should produce a ticket after start()");
    let (a_events_tx, mut a_events_rx) = mpsc::unbounded_channel::<TransportEvent>();
    let transport_a = IrohTransport::new(IrohConfig {
        device_id: "device-A".to_string(),
        device_name: "Device A".to_string(),
        space_id: "test-space-iroh-bidirectional".to_string(),
        secret_key: None,
        peer_addr: None,
        peer_ticket: Some(ticket_b),
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    });
    transport_a
        .start(a_events_tx)
        .await
        .expect("transport A start");
    let (b_got_hello_from, _b_hello_msg) =
        find_message(&mut b_events_rx, EVENT_GUARD, |_from, msg| {
            matches!(msg, LanSyncMessage::Hello { .. })
        })
        .await
        .expect("B must receive a Hello from A (auto-injected by transport)");
    assert_eq!(
        b_got_hello_from, "device-A",
        "B должна получить Hello с from_device_id == device-A"
    );
    let (a_got_hello_from, _a_hello_msg) =
        find_message(&mut a_events_rx, EVENT_GUARD, |_from, msg| {
            matches!(msg, LanSyncMessage::Hello { .. })
        })
        .await
        .expect("A must receive a Hello from B (auto-injected by transport)");
    assert_eq!(
        a_got_hello_from, "device-B",
        "A должна получить Hello с from_device_id == device-B"
    );
    let change_id = format!("reverse-change-{}", generate_id());
    let entity = SyncEntity {
        entity_type: "todo".to_string(),
        id: format!("reverse-entity-{}", generate_id()),
        data: {
            let mut m = serde_json::Map::new();
            m.insert(
                "title".to_string(),
                serde_json::Value::String("B>A reverse direction test".to_string()),
            );
            m
        },
        hlc: "2026-06-17T00:00:00.000Z:000001:device-B".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    let live_change = LanSyncMessage::LiveChange {
        change_id: change_id.clone(),
        entity: entity.clone(),
        origin_device_id: None, // намеренно None — registry должен резолвить
    };
    transport_b
        .send(live_change)
        .expect("B send LiveChange to A");
    let (a_got_from, a_msg) = find_message(&mut a_events_rx, EVENT_GUARD, |_from, msg| {
        matches!(msg, LanSyncMessage::LiveChange { .. })
    })
    .await
    .expect("A must receive LiveChange from B (reverse direction)");
    assert_eq!(
        a_got_from, "device-B",
        "from_device_id должен быть device-B (резолвится через реестр или Hello)"
    );
    assert!(
        matches!(
            &a_msg,
            LanSyncMessage::LiveChange { change_id: cid, entity: ent, .. }
                if *cid == change_id && ent.id == entity.id
        ),
        "expected LiveChange {change_id} for {}, got {a_msg:?}",
        entity.id
    );
    let (backend, conn) = authorized_storage();
    let recipient_transport = transport_a.endpoint_id().unwrap().to_string();
    set_recipient_transport_key(&conn, &recipient_transport, NodeStatus::Active, 1);
    let frame = signed_frame();
    transport_b
        .bind_authenticated_peer(
            "device-A",
            &transport_a
                .endpoint_id()
                .expect("A endpoint id")
                .to_string(),
        )
        .expect("B should bind A's authenticated endpoint");
    let missing_authority = SignedSyncEnvelope::new(
        "test-space-iroh-bidirectional",
        "device-B",
        "device-A",
        1,
        "addressed-without-authority",
        Vec::new(),
        "00",
    );
    let error = transport_b
        .send_to(
            "device-A",
            LanSyncMessage::SignedIntegrationFrame {
                frame: missing_authority,
            },
        )
        .await
        .expect_err("missing outbound authorization must fail closed");
    assert!(error.contains("authorization"));
    transport_b
        .set_outbound_storage(
            Arc::new(TestStorage {
                inner: backend.clone(),
                first_check: None,
                release_first: None,
            }),
            "test-space-iroh-bidirectional",
            "device-B",
        )
        .expect("outbound authorization context");
    transport_b
        .send_to("device-A", LanSyncMessage::SignedIntegrationFrame { frame })
        .await
        .expect("B should address an integration frame to authenticated A");
    let (_, addressed) = find_message(&mut a_events_rx, EVENT_GUARD, |_from, msg| {
        matches!(msg, LanSyncMessage::SignedIntegrationFrame { .. })
    })
    .await
    .expect("A must receive the addressed frame");
    assert!(matches!(
        addressed,
        LanSyncMessage::SignedIntegrationFrame { .. }
    ));
    transport_b
        .set_outbound_storage(
            Arc::new(TestStorage {
                inner: backend.clone(),
                first_check: None,
                release_first: None,
            }),
            "test-space-iroh-bidirectional",
            "device-B",
        )
        .expect("revoking authorization context");
    let first_check = Arc::new(tokio::sync::Notify::new());
    let release_first = Arc::new(tokio::sync::Notify::new());
    transport_b
        .set_outbound_storage(
            Arc::new(TestStorage {
                inner: backend,
                first_check: Some(first_check.clone()),
                release_first: Some(release_first.clone()),
            }),
            "test-space-iroh-bidirectional",
            "device-B",
        )
        .expect("barrier authorization context");
    let pending = tokio::spawn({
        let transport_b = transport_b.clone();
        async move {
            transport_b
                .send_to(
                    "device-A",
                    LanSyncMessage::SignedIntegrationFrame {
                        frame: signed_frame(),
                    },
                )
                .await
        }
    });
    first_check.notified().await;
    set_recipient_transport_key(&conn, "rotated-key", NodeStatus::Active, 1);
    release_first.notify_one();
    let error = pending
        .await
        .unwrap()
        .expect_err("writer must reject a frame after transport key rotation");
    assert!(error.contains("transport identity"));
    set_recipient_transport_key(&conn, "rotated-key", NodeStatus::Revoked, 2);
    let enqueue_error = transport_b
        .send_to(
            "device-A",
            LanSyncMessage::SignedIntegrationFrame {
                frame: signed_frame(),
            },
        )
        .await
        .expect_err("already revoked recipient must be rejected before enqueue");
    assert!(
        enqueue_error.contains("revoked"),
        "expected revoked rejection, got {enqueue_error}"
    );
    assert!(
        find_message(
            &mut a_events_rx,
            Duration::from_millis(200),
            |_from, msg| { matches!(msg, LanSyncMessage::SignedIntegrationFrame { .. }) }
        )
        .await
        .is_none(),
        "revoked frame must not reach peer"
    );
    let foreign = SignedSyncEnvelope::new(
        "test-space-iroh-bidirectional",
        "device-B",
        "device-C",
        1,
        "addressed-foreign",
        Vec::new(),
        "00",
    );
    let error = transport_b
        .send_to(
            "device-A",
            LanSyncMessage::SignedIntegrationFrame { frame: foreign },
        )
        .await
        .expect_err("foreign recipient must be rejected before the wire");
    assert!(error.contains("recipient"));
    transport_a.stop();
    transport_b.stop();
}
