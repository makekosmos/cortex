use super::*;

/// A failed durable enrollment must not start the sync stream or leave a
/// partially trusted peer behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn failed_enrollment_does_not_start_sync_or_admit_peer() {
    let (fixture_a, fixture_b, _) = pending_pairing().await;
    let db = rusqlite::Connection::open(fixture_a.dir.path().join("ark.db")).unwrap();
    db.execute_batch(
        "CREATE TRIGGER reject_device_registry_enrollment
         BEFORE INSERT ON device_registry_projection
         BEGIN SELECT RAISE(ABORT, 'injected enrollment failure'); END;",
    )
    .unwrap();

    let result = handle_request(
        &fixture_a.state,
        Request::AcceptPairing {
            device_id: "device-b".to_string(),
        },
    )
    .await;
    assert!(
        result.is_err(),
        "acceptance must report failed durable enrollment"
    );
    assert_eq!(registry_event_count(&fixture_a.dir), 0);
    assert!(!peer_online(&snapshot(&fixture_a.state).await, "device-b"));
    assert!(!peer_online(&snapshot(&fixture_b.state).await, "device-a"));

    handle_request(&fixture_a.state, Request::StopSync)
        .await
        .unwrap();
    handle_request(&fixture_b.state, Request::StopSync)
        .await
        .unwrap();
}

/// An unreachable endpoint remains a pending attempt until the caller cancels.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_with_pairing_code_to_dead_peer_stays_pending() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    init_device(&state, &fixture.dir).await;
    start_iroh(&state, "device-b", "Device B").await;

    let dead_endpoint = iroh::SecretKey::generate().public();
    let dead_addr =
        iroh::EndpointAddr::new(dead_endpoint).with_ip_addr("127.0.0.1:9".parse().unwrap());
    let dead_ticket = crate::iroh_transport::IrohTransport::ticket_string_for_addr(&dead_addr);

    let reply = handle_request(
        &state,
        Request::ConnectWithPairingCode {
            pairing_code: dead_ticket,
        },
    )
    .await
    .expect("an unreachable peer is a pending attempt, not an RPC error");
    assert_eq!(reply["status"].as_str(), Some("pending"));

    let snapshot = snapshot(&state).await;
    assert_eq!(
        snapshot["outgoing_pairing"]["status"].as_str(),
        Some("pending"),
        "{snapshot}"
    );

    handle_request(&state, Request::CancelPairing)
        .await
        .unwrap();
    handle_request(&state, Request::StopSync).await.unwrap();
}
