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

    let Request::StartSync(params) = request else {
        panic!("expected start_sync, got {request:?}");
    };
    assert!(params.use_iroh);
    assert_eq!(
        params.iroh_peer_ticket.as_deref(),
        Some("endpointsometicketvalue")
    );
    assert!(params.discovery_enabled);
}

#[test]
fn request_deserialization_accepts_code_alias_for_pairing() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "connect_with_pairing_code",
        "code": "endpointdemo123"
    }))
    .expect("code alias should deserialize for pairing requests");

    let Request::ConnectWithPairingCode { pairing_code } = request else {
        panic!("expected connect_with_pairing_code, got {request:?}");
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
            pairing_connect: false,
            discovery_enabled: true,
            // Loopback on purpose: the pairing restart must escalate the
            // bind, not just inherit it (KOS-269).
            bind: SyncBind::Loopback,
            app_version: None,
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
    // KOS-269: pairing is the explicit sync opt-in — even a loopback-bound
    // boot must switch to the LAN bind so the iroh endpoint can reach the
    // remote ticket (and the firewall prompt happens now, not at boot).
    assert_eq!(params.bind, SyncBind::AllInterfaces);
}

/// The sync runtime fixture shared by the bind-escalation tests below:
/// loopback-bound, relay transport selected, no iroh ticket — the state a
/// no-privileges boot leaves behind.
fn loopback_runtime_fixture() -> SyncRuntime {
    let storage = Arc::new(crate::db::SqliteStorageBackend::new(Arc::new(
        StdMutex::new(rusqlite::Connection::open_in_memory().unwrap()),
    )));
    SyncRuntime {
        server: Arc::new(crate::sync_server::SyncServer::new(
            storage.clone() as Arc<dyn crate::sync_server::StorageBackend>
        )),
        storage,
        clients: Arc::new(TokioMutex::new(HashMap::new())),
        relay: None,
        transport_choice: Some(TransportChoice::Relay),
        start_params: SyncStartParams {
            space_id: "space-a".to_string(),
            device_id: "device-a".to_string(),
            device_name: "Device A".to_string(),
            port: Some(21531),
            seed_addresses: None,
            relay_url: None,
            relay_api_key: None,
            auth_secret: None,
            use_iroh: false,
            iroh_peer_ticket: None,
            pairing_connect: false,
            discovery_enabled: false,
            bind: SyncBind::Loopback,
            app_version: None,
        },
        iroh_our_ticket: None,
        beacon: Arc::new(crate::beacon::BroadcastDiscovery::new()),
        space_id: "space-a".to_string(),
        device_id: "device-a".to_string(),
        device_name: "Device A".to_string(),
        auth_secret: None,
        own_addresses: Arc::new(TokioMutex::new(Vec::new())),
    }
}

#[test]
fn show_pairing_code_params_escalate_loopback_to_lan() {
    // KOS-269 round 1: showing our own code is the *other* explicit opt-in.
    // A loopback iroh endpoint advertises only 127.0.0.1, which no remote
    // device can reach — so `show_pairing_code` must restart sync with the
    // same all-interfaces/iroh params as entering a code (minus the peer
    // ticket). The restart itself is covered by the param builder: a real
    // all-interfaces bind must not happen in tests.
    let runtime = loopback_runtime_fixture();
    let params = crate::service::sync::lan_opt_in_params(&runtime, None);
    assert_eq!(params.bind, SyncBind::AllInterfaces);
    assert!(params.use_iroh);
    assert!(params.iroh_peer_ticket.is_none());
    assert_eq!(params.space_id, "space-a");
    assert_eq!(params.device_id, "device-a");
}

#[tokio::test]
async fn show_pairing_code_returns_existing_ticket_without_restart() {
    // A LAN-ready runtime must not restart: the request answers with the
    // ticket it already has.
    let state = test_state();
    let mut runtime = loopback_runtime_fixture();
    runtime.start_params.bind = SyncBind::AllInterfaces;
    runtime.start_params.use_iroh = true;
    runtime.iroh_our_ticket = Some("endpoint-our-ticket".to_string());
    *state.sync.lock().await = Some(Arc::new(runtime));

    let result = handle_request(&state, Request::ShowPairingCode)
        .await
        .expect("show_pairing_code should return the running ticket");
    assert_eq!(result.as_str(), Some("endpoint-our-ticket"));
    // Bind untouched — still the all-interfaces runtime we installed.
    let after = state.sync.lock().await;
    assert_eq!(
        after.as_ref().unwrap().start_params.bind,
        SyncBind::AllInterfaces
    );
}

#[tokio::test]
async fn show_pairing_code_errors_when_sync_is_not_running() {
    let state = test_state();
    let err = handle_request(&state, Request::ShowPairingCode)
        .await
        .expect_err("show_pairing_code must fail without a running sync");
    assert!(err.contains("Sync not running"), "unexpected error: {err}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_sync_with_use_iroh_selects_iroh_transport_and_exposes_ticket() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let dir = &fixture.dir;
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
        Request::StartSync(StartSyncParams {
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
            pairing_connect: false,
            discovery_enabled: false,
            bind: SyncBind::Loopback,
            app_version: None,
        }),
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

#[test]
fn request_deserialization_defaults_iroh_fields_when_absent() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "start_sync",
        "space_id": "space",
        "device_id": "device"
    }))
    .expect("start_sync without iroh fields should still deserialize");

    let Request::StartSync(params) = request else {
        panic!("expected start_sync, got {request:?}");
    };
    assert!(!params.use_iroh);
    assert_eq!(params.iroh_peer_ticket, None);
    assert!(params.discovery_enabled);
    assert_eq!(params.bind, SyncBind::AllInterfaces);
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

    let Request::StartSync(params) = request else {
        panic!("expected start_sync, got {request:?}");
    };
    assert!(!params.discovery_enabled);
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

    let Request::StartSync(params) = request else {
        panic!("expected start_sync, got {request:?}");
    };
    assert_eq!(params.relay_url.as_deref(), Some("ws://127.0.0.1:8765"));
    assert_eq!(params.relay_api_key.as_deref(), Some("key"));
    assert_eq!(params.auth_secret.as_deref(), Some("secret"));
}

// The newtype-over-params-struct variants must keep the exact wire format:
// the params struct fields are the same JSON keys the inline fields had.
#[test]
fn params_struct_variants_keep_their_wire_format() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "start_sync",
        "space_id": "space",
        "device_id": "device",
        "device_name": "Laptop",
        "port": 21531,
        "seed_addresses": ["10.0.0.1:21531"],
        "relay_url": "wss://relay.example",
        "relay_api_key": "key",
        "auth_secret": "secret",
        "use_iroh": true,
        "iroh_peer_ticket": "ticket",
        "discovery_enabled": false,
        "bind": "loopback"
    }))
    .expect("start_sync params must deserialize through the newtype variant");
    let Request::StartSync(params) = request else {
        panic!("expected start_sync, got {request:?}");
    };
    assert_eq!(params.space_id, "space");
    assert_eq!(params.device_id, "device");
    assert_eq!(params.device_name.as_deref(), Some("Laptop"));
    assert_eq!(params.port, Some(21531));
    assert_eq!(
        params.seed_addresses,
        Some(vec!["10.0.0.1:21531".to_string()])
    );
    assert_eq!(params.relay_url.as_deref(), Some("wss://relay.example"));
    assert_eq!(params.relay_api_key.as_deref(), Some("key"));
    assert_eq!(params.auth_secret.as_deref(), Some("secret"));
    assert!(params.use_iroh);
    assert_eq!(params.iroh_peer_ticket.as_deref(), Some("ticket"));
    assert!(!params.discovery_enabled);
    assert_eq!(params.bind, SyncBind::Loopback);

    let request = serde_json::from_value::<Request>(json!({
        "operation": "external_refs.upsert",
        "connectorId": "connector",
        "accountId": "account",
        "externalType": "issue",
        "externalId": "ext-1",
        "objectId": "obj-1",
        "revision": "rev-1",
        "hash": "hash-1",
        "state": "linked"
    }))
    .expect("external_refs.upsert params must deserialize through the newtype variant");
    let Request::ExternalRefsUpsert(params) = request else {
        panic!("expected external_refs.upsert, got {request:?}");
    };
    assert_eq!(params.connector_id, "connector");
    assert_eq!(params.account_id, "account");
    assert_eq!(params.external_type, "issue");
    assert_eq!(params.external_id, "ext-1");
    assert_eq!(params.object_id, "obj-1");
    assert_eq!(params.revision.as_deref(), Some("rev-1"));
    assert_eq!(params.hash.as_deref(), Some("hash-1"));
    assert_eq!(params.state, "linked");

    let request = serde_json::from_value::<Request>(json!({
        "operation": "integration.acquire_refresh_lease",
        "integration_id": "integration",
        "holder_node_id": "node",
        "credential_generation": 3,
        "now_ms": 1000,
        "ttl_ms": 100,
        "expected_fencing_token": 2,
        "device_id": "node"
    }))
    .expect("acquire_refresh_lease params must deserialize through the newtype variant");
    let Request::IntegrationAcquireRefreshLease(params) = request else {
        panic!("expected integration.acquire_refresh_lease, got {request:?}");
    };
    assert_eq!(params.integration_id, "integration");
    assert_eq!(params.holder_node_id, "node");
    assert_eq!(params.credential_generation, 3);
    assert_eq!(params.now_ms, 1000);
    assert_eq!(params.ttl_ms, 100);
    assert_eq!(params.expected_fencing_token, 2);
    assert_eq!(params.device_id, "node");
}
