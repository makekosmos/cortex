use super::*;

/// «Принять» completes the pairing: both snapshots go online, the endpoint
/// key persists, and an Engine restart reconnects with no fresh consent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn accept_pairing_connects_persists_and_survives_restart() {
    let (fixture_a, fixture_b, ticket) = pending_pairing().await;
    let state_a = fixture_a.state.clone();
    let state_b = fixture_b.state.clone();

    let reply = handle_request(
        &state_a,
        Request::AcceptPairing {
            device_id: "device-b".to_string(),
        },
    )
    .await
    .expect("accept_pairing must succeed for a pending request");
    assert_eq!(reply["status"].as_str(), Some("connected"));
    assert_eq!(reply["device_name"].as_str(), Some("Device B"));

    wait_until("both sides online", async || {
        peer_online(&snapshot(&state_a).await, "device-b")
            && peer_online(&snapshot(&state_b).await, "device-a")
    })
    .await;

    let keys = transport_keys(&state_a).await;
    assert!(
        keys.as_str().is_some_and(|raw| raw.contains("device-b")),
        "accepted pairing must persist the endpoint key: {keys}"
    );
    wait_until("the responder's accept resolves B's attempt", async || {
        snapshot(&state_b).await["outgoing_pairing"]["status"].as_str() == Some("connected")
    })
    .await;
    // Both devices author one enrollment event; registry admission exchanges
    // the peer's event before the data channel becomes ready.
    wait_until("A receives B's enrollment event", async || {
        registry_event_count(&fixture_a.dir) == 2
    })
    .await;
    assert_eq!(registry_event_count(&fixture_a.dir), 2);

    // Restart the initiator's sync the way boot restore does (same ticket):
    // the stored endpoint key re-admits it on A with no new consent prompt.
    handle_request(&state_b, Request::StopSync).await.unwrap();
    handle_request(
        &state_b,
        Request::StartSync(iroh_params("device-b", "Device B", Some(ticket))),
    )
    .await
    .unwrap();
    wait_until("B re-authenticates on A without consent", async || {
        peer_online(&snapshot(&state_a).await, "device-b")
    })
    .await;
    wait_until("reconnect preserves the enrollment registry", async || {
        registry_event_count(&fixture_a.dir) == 2
    })
    .await;
    assert_eq!(registry_event_count(&fixture_a.dir), 2);

    // Exercise the production Engine -> DB actor -> service fanout path.
    // Transport-only tests call publish_registry_event directly and therefore
    // cannot catch a missing command-to-relay connection.
    handle_request(
        &state_a,
        Request::DeviceRegistryRename {
            target_device_id: ark_domain::device_registry::DeviceId::parse("device-b").unwrap(),
            display_name: ark_domain::device_registry::DisplayName::parse("Renamed by device A")
                .unwrap(),
        },
    )
    .await
    .expect("Engine registry rename must commit");
    wait_until("Engine rename reaches paired Iroh peer", async || {
        let registry = handle_request(&state_b, Request::DeviceRegistrySnapshot)
            .await
            .unwrap();
        registry["devices"].as_array().is_some_and(|devices| {
            devices.iter().any(|device| {
                device["deviceId"] == "device-b" && device["displayName"] == "Renamed by device A"
            })
        })
    })
    .await;
    wait_until("A commits the peer rename event", async || {
        registry_event_count(&fixture_a.dir) == 3
    })
    .await;
    assert_eq!(registry_event_count(&fixture_a.dir), 3);
    let snapshot_a = snapshot(&state_a).await;
    assert!(
        pending_request(&snapshot_a, "device-b").is_none(),
        "a paired device must not trigger consent again: {snapshot_a}"
    );
    // Boot-style restores replay the ticket but are NOT a new attempt —
    // no outgoing state may surface on the initiator's side either.
    let snapshot_b = snapshot(&state_b).await;
    assert!(
        snapshot_b["outgoing_pairing"].is_null(),
        "a restored start must not resurface an outgoing attempt: {snapshot_b}"
    );

    // «Отключить» then re-entering the same code must ask again — the
    // responder consents explicitly on each re-pair.
    handle_request(
        &state_a,
        Request::DisconnectPeer {
            device_id: "device-b".to_string(),
        },
    )
    .await
    .unwrap();
    wait_until("A drops B", async || {
        !peer_online(&snapshot(&state_a).await, "device-b")
    })
    .await;
    wait_until("A sees the re-pair request", async || {
        pending_request(&snapshot(&state_a).await, "device-b").is_some()
    })
    .await;
    handle_request(
        &state_a,
        Request::AcceptPairing {
            device_id: "device-b".to_string(),
        },
    )
    .await
    .unwrap();
    wait_until("B is online again after re-consent", async || {
        peer_online(&snapshot(&state_a).await, "device-b")
            && peer_online(&snapshot(&state_b).await, "device-a")
    })
    .await;

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}
