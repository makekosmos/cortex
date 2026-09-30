use super::*;

// The `{"ok","data","error"}` response envelope now lives in the Engine's
// `ark_host` adapter; wire-shape coverage for it moved to
// `runtime/src/ark_host.rs` tests.

#[test]
fn request_deserialization_ignores_optional_id_field() {
    let request = serde_json::from_value::<Request>(json!({
        "id": "req-3",
        "operation": "get_host_device_name"
    }))
    .expect("request id must be backward-compatible metadata");

    assert!(matches!(request, Request::GetHostDeviceName));
}

#[test]
fn request_deserialization_accepts_iroh_config() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "start_sync",
        "space_id": "space",
        "device_id": "device",
        "use_iroh": true,
        "iroh_peer_ticket": "endpointsometicketvalue"
    }))
    .expect("iroh config should be accepted by the request schema");

    let Request::StartSync {
        use_iroh,
        iroh_peer_ticket,
        discovery_enabled,
        ..
    } = request
    else {
        unreachable!("expected start_sync");
    };
    assert!(use_iroh);
    assert_eq!(iroh_peer_ticket.as_deref(), Some("endpointsometicketvalue"));
    assert!(discovery_enabled);
}

#[test]
fn request_deserialization_accepts_code_alias_for_pairing() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "connect_with_pairing_code",
        "code": "endpointdemo123"
    }))
    .expect("code alias should deserialize for pairing requests");

    let Request::ConnectWithPairingCode { pairing_code } = request else {
        unreachable!("expected connect_with_pairing_code");
    };
    assert_eq!(pairing_code, "endpointdemo123");
}

#[test]
fn request_deserialization_accepts_signed_sync_send() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "integration.send_signed_sync",
        "frame": {
            "space_id": "space-a",
            "origin_node_id": "node-a",
            "recipient_node_id": "node-b",
            "key_epoch": 1,
            "message_id": "message-1",
            "payload": [],
            "signature": "00"
        }
    }))
    .expect("signed sync send should deserialize");

    assert!(matches!(request, Request::IntegrationSendSignedSync { .. }));
}

#[test]
fn pairing_restart_params_force_iroh_and_replace_ticket() {
    let storage = Arc::new(crate::db::SqliteStorageBackend::new(Arc::new(
        StdMutex::new(rusqlite::Connection::open_in_memory().unwrap()),
    )));
    let runtime = SyncRuntime {
        server: Arc::new(crate::sync_server::SyncServer::new(
            storage.clone() as Arc<dyn crate::sync_server::StorageBackend>
        )),
        storage,
        clients: Arc::new(TokioMutex::new(HashMap::new())),
        relay: None,
        transport_choice: Some(TransportChoice::None),
        start_params: SyncStartParams {
            space_id: "space-a".to_string(),
            device_id: "device-a".to_string(),
            device_name: "Device A".to_string(),
            port: Some(21531),
            seed_addresses: Some(vec!["127.0.0.1:21531".to_string()]),
            relay_url: Some("ws://relay.example".to_string()),
            relay_api_key: Some("relay-key".to_string()),
            auth_secret: Some("secret".to_string()),
            use_iroh: false,
            iroh_peer_ticket: None,
            discovery_enabled: true,
            bind: SyncBind::AllInterfaces,
        },
        iroh_our_ticket: None,
        beacon: Arc::new(crate::beacon::BroadcastDiscovery::new()),
        space_id: "space-a".to_string(),
        device_id: "device-a".to_string(),
        device_name: "Device A".to_string(),
        auth_secret: Some("secret".to_string()),
        own_addresses: Arc::new(TokioMutex::new(Vec::new())),
    };

    let params = build_pairing_restart_params(&runtime, "  endpointdemo123  ");
    assert!(params.use_iroh);
    assert_eq!(params.iroh_peer_ticket.as_deref(), Some("endpointdemo123"));
    assert_eq!(params.relay_url.as_deref(), Some("ws://relay.example"));
    assert_eq!(params.space_id, "space-a");
    assert_eq!(params.device_id, "device-a");
}

#[cfg(feature = "iroh-spike")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_sync_with_use_iroh_selects_iroh_transport_and_exposes_ticket() {
    let state = test_state();
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    // Before start_sync, our ticket must be unavailable.
    let before = handle_request(&state, Request::GetOwnIrohTicket)
        .await
        .unwrap();
    assert_eq!(before, Value::Null);

    let port = {
        // Bind an ephemeral port for the LAN/WS server side of
        // start_sync so this test doesn't collide with LAN_SYNC_PORT
        // across parallel test runs.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let start_result = handle_request(
        &state,
        Request::StartSync {
            space_id: "iroh-space".to_string(),
            device_id: "device-iroh".to_string(),
            device_name: Some("Iroh Device".to_string()),
            port: Some(port),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
            discovery_enabled: false,
            bind: SyncBind::Loopback,
        },
    )
    .await;

    start_result.expect("explicit discovery opt-out must avoid the shared beacon port");
    let ticket = handle_request(&state, Request::GetOwnIrohTicket)
        .await
        .expect("get_own_iroh_ticket should succeed once iroh transport is running");
    assert!(
        ticket.as_str().is_some_and(|s| !s.is_empty()),
        "expected a non-empty iroh ticket string, got {ticket:?}"
    );
    handle_request(&state, Request::StopSync).await.unwrap();
}

#[cfg(not(feature = "iroh-spike"))]
#[tokio::test]
async fn start_sync_with_use_iroh_fails_gracefully_without_iroh_spike_feature() {
    let state = test_state();
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("ark.db");
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.to_string_lossy().to_string(),
        },
    )
    .await
    .unwrap();

    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let result = handle_request(
        &state,
        Request::StartSync {
            space_id: "iroh-space".to_string(),
            device_id: "device-iroh".to_string(),
            device_name: Some("Iroh Device".to_string()),
            port: Some(port),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: true,
            iroh_peer_ticket: None,
            discovery_enabled: false,
            bind: SyncBind::Loopback,
        },
    )
    .await;

    assert!(
        result.is_err(),
        "use_iroh must fail with a clear error when built without iroh-spike, not silently no-op"
    );

    handle_request(&state, Request::StopSync).await.unwrap();
}

#[test]
fn request_deserialization_defaults_iroh_fields_when_absent() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "start_sync",
        "space_id": "space",
        "device_id": "device"
    }))
    .expect("start_sync without iroh fields should still deserialize");

    let Request::StartSync {
        use_iroh,
        iroh_peer_ticket,
        discovery_enabled,
        bind,
        ..
    } = request
    else {
        unreachable!("expected start_sync");
    };
    assert!(!use_iroh);
    assert_eq!(iroh_peer_ticket, None);
    assert!(discovery_enabled);
    assert_eq!(bind, SyncBind::AllInterfaces);
}

#[test]
fn request_deserialization_accepts_discovery_opt_out() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "start_sync",
        "space_id": "space",
        "device_id": "device",
        "discovery_enabled": false
    }))
    .expect("discovery opt-out should deserialize");

    let Request::StartSync {
        discovery_enabled, ..
    } = request
    else {
        unreachable!("expected start_sync");
    };
    assert!(!discovery_enabled);
}

#[test]
fn request_deserialization_accepts_relay_and_auth_config() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "start_sync",
        "space_id": "space",
        "device_id": "device",
        "relay_url": "ws://127.0.0.1:8765",
        "relay_api_key": "key",
        "auth_secret": "secret"
    }))
    .expect("relay config should be accepted by the request schema");

    let Request::StartSync {
        relay_url,
        relay_api_key,
        auth_secret,
        ..
    } = request
    else {
        unreachable!("expected start_sync");
    };
    assert_eq!(relay_url.as_deref(), Some("ws://127.0.0.1:8765"));
    assert_eq!(relay_api_key.as_deref(), Some("key"));
    assert_eq!(auth_secret.as_deref(), Some("secret"));
}
