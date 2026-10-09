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

    // «Показать код» on A: returns the ticket and opens A's first-contact
    // window — the same RPC the Manager fires when the pairing card opens.
    let ticket = handle_request(&state_a, Request::ShowPairingCode)
        .await
        .unwrap()
        .as_str()
        .expect("device A must expose an iroh ticket")
        .to_string();

    // The exact RPC the Manager sends when «Подключить» is clicked.
    let reply = handle_request(
        &state_b,
        Request::ConnectWithPairingCode {
            pairing_code: ticket.clone(),
        },
    )
    .await
    .expect("connect_with_pairing_code must accept a valid ticket");
    assert_eq!(reply["status"].as_str(), Some("connected"));
    assert_eq!(reply["device_id"].as_str(), Some("device-a"));
    assert_eq!(reply["device_name"].as_str(), Some("Device A"));

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

    // «Отключить» must drop the transport peer too: B's snapshot stops
    // reporting A online, and the paired endpoint key is forgotten.
    handle_request(
        &state_b,
        Request::DisconnectPeer {
            device_id: "device-a".to_string(),
        },
    )
    .await
    .unwrap();
    let snapshot_b = handle_request(&state_b, Request::GetSyncSnapshot)
        .await
        .unwrap();
    assert!(
        !peer_online(&snapshot_b, "device-a"),
        "a removed iroh peer must not keep showing online: {snapshot_b}"
    );
    let keys = handle_request(
        &state_b,
        Request::GetSyncKv {
            key: "sync.peer_transport_keys".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(
        keys.is_null() || !keys.as_str().is_some_and(|raw| raw.contains("device-a")),
        "removed peer must lose its stored endpoint key: {keys}"
    );

    // Re-pairing the same code is explicit re-consent: B enters A's code
    // again and the removed block lifts for the ticketed endpoint.
    let repair = handle_request(
        &state_b,
        Request::ConnectWithPairingCode {
            pairing_code: ticket,
        },
    )
    .await;
    let repair_deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut repaired = matches!(
        repair,
        Ok(ref value) if value["device_id"].as_str() == Some("device-a")
    );
    while !repaired {
        assert!(
            std::time::Instant::now() < repair_deadline,
            "timed out waiting for B to re-authenticate A after re-pair"
        );
        let snapshot_b = handle_request(&state_b, Request::GetSyncSnapshot)
            .await
            .unwrap();
        repaired = peer_online(&snapshot_b, "device-a");
        if !repaired {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}

/// First contact is only admitted while a pairing window is open: a peer
/// that learned our ticket (it stays valid forever) but connects while we
/// never showed a code must not get trusted — its Hello is dropped and it
/// never appears in our snapshot.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hello_without_pairing_window_is_not_trusted() {
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

    for (state, id, name) in [
        (&state_a, "device-a", "Device A"),
        (&state_b, "device-b", "Device B"),
    ] {
        handle_request(
            state,
            Request::StartSync(StartSyncParams {
                space_id: "pair-space".to_string(),
                device_id: id.to_string(),
                device_name: Some(name.to_string()),
                port: Some(free_port()),
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

    // Passive ticket read — `GetOwnIrohTicket` opens no pairing window.
    let ticket = handle_request(&state_a, Request::GetOwnIrohTicket)
        .await
        .unwrap()
        .as_str()
        .expect("device A must expose an iroh ticket")
        .to_string();

    // B dials A's ticket (its own connect opens B's window pinned to A) —
    // A never opened one, so B's Hello must not authenticate at A.
    let _ = handle_request(
        &state_b,
        Request::ConnectWithPairingCode {
            pairing_code: ticket,
        },
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let snapshot_a = handle_request(&state_a, Request::GetSyncSnapshot)
        .await
        .unwrap();
    let trusted = snapshot_a["peers"].as_array().is_some_and(|peers| {
        peers.iter().any(|peer| {
            peer["device_id"].as_str() == Some("device-b")
                && peer["status"].as_str() == Some("online")
        })
    });
    assert!(
        !trusted,
        "an endpoint with no open pairing window must not get trusted: {snapshot_a}"
    );
    let keys = handle_request(
        &state_a,
        Request::GetSyncKv {
            key: "sync.peer_transport_keys".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(
        keys.is_null() || !keys.as_str().is_some_and(|raw| raw.contains("device-b")),
        "no pairing window → no persisted endpoint key: {keys}"
    );

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
