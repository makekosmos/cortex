use super::*;

/// KOS-367: the Manager pairing flow end-to-end at the service level —
/// device A runs sync and hands out a ticket, device B consumes it through
/// the same `connect_with_pairing_code` request the UI sends, and both
/// snapshots must then report the other device online. On main the inbound
/// Hello is dropped by the transport-key gate in `handle_event`, so neither
/// side ever authenticates and the UI just shows nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_with_pairing_code_authenticates_both_sides() {
    let fixture_a = service_fixture();
    let fixture_b = service_fixture();
    let state_a = fixture_a.state.clone();
    let state_b = fixture_b.state.clone();

    for (state, dir) in [(&state_a, &fixture_a.dir), (&state_b, &fixture_b.dir)] {
        let db_path = dir.path().join("ark.db");
        handle_request(
            state,
            Request::Init {
                db_path: db_path.to_string_lossy().to_string(),
            },
        )
        .await
        .unwrap();
    }

    let free_port = || {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    };

    async fn start(state: &Arc<ServiceState>, device_id: &str, name: &str, port: u16) {
        handle_request(
            state,
            Request::StartSync(StartSyncParams {
                space_id: "pair-space".to_string(),
                device_id: device_id.to_string(),
                device_name: Some(name.to_string()),
                port: Some(port),
                seed_addresses: None,
                relay_url: None,
                relay_api_key: None,
                auth_secret: None,
                use_iroh: true,
                iroh_peer_ticket: None,
                discovery_enabled: false,
                bind: SyncBind::Loopback,
                app_version: None,
            }),
        )
        .await
        .unwrap();
    }

    start(&state_a, "device-a", "Device A", free_port()).await;
    start(&state_b, "device-b", "Device B", free_port()).await;

    let ticket = handle_request(&state_a, Request::GetOwnIrohTicket)
        .await
        .unwrap()
        .as_str()
        .expect("device A must expose an iroh ticket")
        .to_string();

    // The exact RPC the Manager sends when «Подключить» is clicked.
    handle_request(
        &state_b,
        Request::ConnectWithPairingCode {
            pairing_code: ticket,
        },
    )
    .await
    .expect("connect_with_pairing_code must accept a valid ticket");

    let peer_online = |snapshot: &Value, device_id: &str| {
        snapshot["peers"].as_array().is_some_and(|peers| {
            peers.iter().any(|peer| {
                peer["device_id"].as_str() == Some(device_id)
                    && peer["status"].as_str() == Some("online")
            })
        })
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let (mut a_sees_b, mut b_sees_a) = (false, false);
    while !(a_sees_b && b_sees_a) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for both peers to authenticate (a_sees_b={a_sees_b}, \
             b_sees_a={b_sees_a})"
        );
        let snapshot_a = handle_request(&state_a, Request::GetSyncSnapshot)
            .await
            .unwrap();
        let snapshot_b = handle_request(&state_b, Request::GetSyncSnapshot)
            .await
            .unwrap();
        a_sees_b = peer_online(&snapshot_a, "device-b");
        b_sees_a = peer_online(&snapshot_b, "device-a");
        if !(a_sees_b && b_sees_a) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}

/// KOS-367: a syntactically valid ticket whose peer never answers must come
/// back as an error, not a silent Ok — the dial loop keeps retrying in the
/// background, but the UI gets its banner and the button unlocks.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_with_pairing_code_times_out_on_silent_peer() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    handle_request(
        &state,
        Request::StartSync(StartSyncParams {
            space_id: "pair-space".to_string(),
            device_id: "device-b".to_string(),
            device_name: Some("Device B".to_string()),
            port: Some(port),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
            discovery_enabled: false,
            bind: SyncBind::Loopback,
            app_version: None,
        }),
    )
    .await
    .unwrap();

    // A ticket that decodes fine but whose endpoint never listens.
    let dead_endpoint = iroh::SecretKey::generate().public();
    let dead_addr =
        iroh::EndpointAddr::new(dead_endpoint).with_ip_addr("127.0.0.1:9".parse().unwrap());
    let dead_ticket = crate::iroh_transport::IrohTransport::ticket_string_for_addr(&dead_addr);

    let started = std::time::Instant::now();
    let error = handle_request(
        &state,
        Request::ConnectWithPairingCode {
            pairing_code: dead_ticket,
        },
    )
    .await
    .expect_err("a peer that never answers must be an error, not silent Ok");
    assert!(
        error.contains("did not respond"),
        "unexpected error text: {error}"
    );
    // The wait is bounded — the restart (endpoint bind + `our_ticket`
    // relay-homing timeout) plus the connect budget, never a hang.
    assert!(started.elapsed() < std::time::Duration::from_secs(30));

    handle_request(&state, Request::StopSync).await.unwrap();
}

/// The exact ticket from the KOS-367 report must decode to the reported
/// EndpointId — the wire format was never the problem.
#[test]
fn reported_ticket_decodes() {
    let addr = crate::iroh_transport::from_ticket(
        "endpointaa2bzzly2lh3s442nsu6ttlbfvozdcyk5xj6qzw7qcf7xbuxv5mxubaaenuhi5dqom5c6l3fovrtcljrfzzgk3dbpexg4mbonfzg62bonruw42zof4aqacqaaabndmydaeaj3liztwc3uaqbadakqaky2gzqg",
    )
    .expect("the reported ticket is a valid EndpointTicket");
    assert_eq!(
        addr.id.to_string(),
        "341ce578d2cfb9739a6ca9e9cd612d5d918b0aedd3e866df808bfb8697af597a"
    );
}
