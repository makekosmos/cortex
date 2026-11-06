//! Cold-start must never wait on the network: with the iroh endpoint
//! homing on an unreachable relay, `start_sync` returns promptly and
//! follow-up requests are not queued behind the pending ticket. The relay
//! URL points at a closed loopback port — connection refused immediately,
//! relay homing never completes — so `our_ticket()` stays pending until
//! its internal timeout returns the best-effort address.

use std::time::Duration;

use super::*;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_sync_with_unreachable_relay_does_not_block_the_worker() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().to_string())
        .await
        .unwrap();

    let started = std::time::Instant::now();
    handle_request(
        &state,
        Request::StartSync(StartSyncParams {
            space_id: "offline-space".to_string(),
            device_id: "device-offline".to_string(),
            device_name: Some("Offline Device".to_string()),
            port: Some(free_port()),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
            // Closed loopback port: the endpoint never reaches a relay, so
            // `our_ticket()`'s homing wait stays pending on the spawned task.
            iroh_relay_url: Some("https://127.0.0.1:1".to_string()),
            pairing_connect: false,
            discovery_enabled: false,
            bind: SyncBind::Loopback,
            app_version: None,
        }),
    )
    .await
    .expect("start_sync must succeed even when the relay is unreachable");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "start_sync waited on relay homing: {:?}",
        started.elapsed()
    );

    // While the ticket task is still pending, requests run and report
    // "no ticket yet" instead of blocking.
    let read_started = std::time::Instant::now();
    let ticket = handle_request(&state, Request::GetOwnIrohTicket)
        .await
        .unwrap();
    let snapshot = handle_request(&state, Request::GetSyncSnapshot)
        .await
        .unwrap();
    assert!(
        read_started.elapsed() < Duration::from_millis(200),
        "local reads stalled behind the pending ticket: {:?}",
        read_started.elapsed()
    );
    assert_eq!(ticket, Value::Null);
    assert_eq!(snapshot["pairing_available"], json!(false));
    assert_eq!(snapshot["running"], json!(true));

    // Once the homing timeout inside `our_ticket()` elapses, the channel
    // still fills with the best-effort ticket — start_sync is not
    // "sync broken", just not-yet-homed.
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let ticket = handle_request(&state, Request::GetOwnIrohTicket)
                .await
                .unwrap();
            if ticket.as_str().is_some_and(|s| !s.is_empty()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("ticket must resolve to the best-effort addr after the homing timeout");

    handle_request(&state, Request::StopSync).await.unwrap();
}
