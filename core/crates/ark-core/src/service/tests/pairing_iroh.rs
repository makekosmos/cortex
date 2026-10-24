use super::*;

/// KOS-369: the Manager pairing flow end-to-end at the service level, now
/// gated on an explicit «Принять / Отклонить» consent instead of a timed
/// pairing window. Device A runs sync and exposes a ticket, device B
/// consumes it through `connect_with_pairing_code`; B's RPC answers
/// `pending` immediately, A's snapshot lists an incoming request, and only
/// `accept_pairing` turns it into a trusted peer.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn init_device(state: &Arc<ServiceState>, dir: &crate::test_support::TestTempDir) {
    initialize_test_database(
        state,
        dir.path().join("ark.db").to_string_lossy().to_string(),
    )
    .await
    .unwrap();
}

fn iroh_params(device_id: &str, name: &str, ticket: Option<String>) -> StartSyncParams {
    StartSyncParams {
        space_id: "pair-space".to_string(),
        device_id: device_id.to_string(),
        device_name: Some(name.to_string()),
        port: Some(free_port()),
        seed_addresses: None,
        relay_url: None,
        relay_api_key: None,
        auth_secret: None,
        use_iroh: true,
        iroh_peer_ticket: ticket,
        iroh_relay_url: None,
        pairing_connect: false,
        discovery_enabled: false,
        bind: SyncBind::Loopback,
        app_version: None,
    }
}

async fn start_iroh(state: &Arc<ServiceState>, device_id: &str, name: &str) {
    handle_request(
        state,
        Request::StartSync(iroh_params(device_id, name, None)),
    )
    .await
    .unwrap();
}

async fn ticket_of(state: &Arc<ServiceState>) -> String {
    handle_request(state, Request::GetOwnIrohTicket)
        .await
        .unwrap()
        .as_str()
        .expect("device must expose an iroh ticket")
        .to_string()
}

async fn snapshot(state: &Arc<ServiceState>) -> Value {
    handle_request(state, Request::GetSyncSnapshot)
        .await
        .unwrap()
}

fn peer_online(snapshot: &Value, device_id: &str) -> bool {
    snapshot["peers"].as_array().is_some_and(|peers| {
        peers.iter().any(|peer| {
            peer["device_id"].as_str() == Some(device_id)
                && peer["status"].as_str() == Some("online")
        })
    })
}

fn pending_request<'a>(snapshot: &'a Value, device_id: &str) -> Option<&'a Value> {
    snapshot["incoming_pairing_requests"]
        .as_array()?
        .iter()
        .find(|request| request["device_id"].as_str() == Some(device_id))
}

async fn wait_until(label: &str, mut condition: impl AsyncFnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !condition().await {
        assert!(std::time::Instant::now() < deadline, "timed out: {label}");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

async fn transport_keys(state: &Arc<ServiceState>) -> Value {
    handle_request(
        state,
        Request::GetSyncKv {
            key: "sync.peer_transport_keys".to_string(),
        },
    )
    .await
    .unwrap()
}

fn registry_event_count(dir: &crate::test_support::TestTempDir) -> usize {
    ark_store::test_support::DeviceRegistryFixture::open(&dir.path().join("ark.db"))
        .unwrap()
        .inspect()
        .unwrap()
        .events
        .len()
}

/// A + B running iroh, B has consumed A's ticket; A's snapshot must be
/// carrying the pending consent request before the caller proceeds.
/// Returns the fixtures and the ticket B dialed.
async fn pending_pairing() -> (ServiceFixture, ServiceFixture, String) {
    let fixture_a = service_fixture();
    let fixture_b = service_fixture();
    init_device(&fixture_a.state, &fixture_a.dir).await;
    init_device(&fixture_b.state, &fixture_b.dir).await;
    start_iroh(&fixture_a.state, "device-a", "Device A").await;
    start_iroh(&fixture_b.state, "device-b", "Device B").await;
    let ticket = ticket_of(&fixture_a.state).await;

    let reply = handle_request(
        &fixture_b.state,
        Request::ConnectWithPairingCode {
            pairing_code: ticket.clone(),
        },
    )
    .await
    .expect("connect_with_pairing_code must not fail while consent is pending");
    assert_eq!(reply["status"].as_str(), Some("pending"));

    wait_until("A sees B's consent request", async || {
        pending_request(&snapshot(&fixture_a.state).await, "device-b").is_some()
    })
    .await;
    let snapshot_b = snapshot(&fixture_b.state).await;
    assert!(
        !peer_online(&snapshot_b, "device-a"),
        "the dialer must not admit the responder before it accepts pairing: {snapshot_b}"
    );
    assert_eq!(registry_event_count(&fixture_a.dir), 0);
    assert_eq!(registry_event_count(&fixture_b.dir), 0);
    (fixture_a, fixture_b, ticket)
}

/// Entering a code returns `pending` promptly (the human decides later), and
/// the responder's snapshot surfaces the requester by name and platform —
/// without «Подключить устройство» ever being opened on the responder.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_with_pairing_code_returns_pending_awaiting_consent() {
    let fixture_a = service_fixture();
    let fixture_b = service_fixture();
    init_device(&fixture_a.state, &fixture_a.dir).await;
    init_device(&fixture_b.state, &fixture_b.dir).await;
    start_iroh(&fixture_a.state, "device-a", "Device A").await;
    start_iroh(&fixture_b.state, "device-b", "Device B").await;
    let ticket = ticket_of(&fixture_a.state).await;

    let started = std::time::Instant::now();
    let reply = handle_request(
        &fixture_b.state,
        Request::ConnectWithPairingCode {
            pairing_code: ticket,
        },
    )
    .await
    .expect("connect_with_pairing_code must not fail while consent is pending");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the RPC must return promptly — the human decision is async"
    );
    assert_eq!(reply["status"].as_str(), Some("pending"));

    wait_until("A's snapshot lists the pending request", async || {
        pending_request(&snapshot(&fixture_a.state).await, "device-b").is_some()
    })
    .await;
    let request = pending_request(&snapshot(&fixture_a.state).await, "device-b")
        .expect("pending request")
        .clone();
    assert_eq!(request["device_name"].as_str(), Some("Device B"));
    assert!(
        request["platform"].as_str().is_some_and(|p| !p.is_empty()),
        "the consent prompt needs the platform: {request}"
    );

    // Pending is not trust: no online peer and no persisted endpoint key.
    let snapshot_a = snapshot(&fixture_a.state).await;
    assert!(!peer_online(&snapshot_a, "device-b"), "{snapshot_a}");
    let keys = transport_keys(&fixture_a.state).await;
    assert!(
        keys.is_null() || !keys.as_str().is_some_and(|raw| raw.contains("device-b")),
        "an unanswered request must persist nothing: {keys}"
    );

    handle_request(&fixture_a.state, Request::StopSync)
        .await
        .unwrap();
    handle_request(&fixture_b.state, Request::StopSync)
        .await
        .unwrap();
}

/// «Отклонить» rejects the connection at the protocol level: the initiator
/// learns `declined` in its snapshot, the responder persists nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decline_pairing_rejects_initiator_and_persists_nothing() {
    let (fixture_a, fixture_b, _ticket) = pending_pairing().await;
    let state_a = fixture_a.state.clone();
    let state_b = fixture_b.state.clone();

    let reply = handle_request(
        &state_a,
        Request::DeclinePairing {
            device_id: "device-b".to_string(),
        },
    )
    .await
    .expect("decline_pairing must succeed for a pending request");
    assert_eq!(reply["status"].as_str(), Some("declined"));

    wait_until("B observes the explicit decline", async || {
        snapshot(&state_b).await["outgoing_pairing"]["status"].as_str() == Some("declined")
    })
    .await;

    let snapshot_a = snapshot(&state_a).await;
    assert!(
        pending_request(&snapshot_a, "device-b").is_none(),
        "{snapshot_a}"
    );
    assert!(!peer_online(&snapshot_a, "device-b"), "{snapshot_a}");
    let keys = transport_keys(&state_a).await;
    assert!(
        keys.is_null() || !keys.as_str().is_some_and(|raw| raw.contains("device-b")),
        "decline must persist no endpoint key: {keys}"
    );

    // On the initiator the pre-consented trust is rolled back too: A leaves
    // B's peer list and keeps no stored key.
    let snapshot_b = snapshot(&state_b).await;
    assert!(
        !peer_online(&snapshot_b, "device-a"),
        "a declined responder must not stay online on the initiator: {snapshot_b}"
    );
    let keys_b = transport_keys(&state_b).await;
    assert!(
        keys_b.is_null() || !keys_b.as_str().is_some_and(|raw| raw.contains("device-a")),
        "decline must roll back the persisted endpoint key: {keys_b}"
    );
    let peers_kv = handle_request(
        &state_a,
        Request::GetSyncKv {
            key: "sync.peers".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(
        peers_kv.is_null()
            || !peers_kv
                .as_str()
                .is_some_and(|raw| raw.contains("device-b")),
        "decline must persist no peer record: {peers_kv}"
    );
    assert_eq!(registry_event_count(&fixture_a.dir), 0);
    assert_eq!(registry_event_count(&fixture_b.dir), 0);

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}

/// While consent is pending the unknown endpoint is walled off both ways:
/// its pushes are dropped and our broadcasts never reach it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pending_pairing_exchanges_no_data() {
    let (fixture_a, fixture_b, _ticket) = pending_pairing().await;
    let state_a = fixture_a.state.clone();
    let state_b = fixture_b.state.clone();

    let entity = |id: &str| {
        let mut object = canonical_task_object(Some("0.1.0"), canonical_task_props());
        object.id = id.to_string();
        let value = serde_json::to_value(&object).unwrap();
        let Value::Object(mut data) = value else {
            panic!("expected JSON object, got {value:?}")
        };
        data.remove("id");
        SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data,
            hlc: "2026-10-09T00:00:00.000Z:000001:src".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    };

    handle_request(
        &state_b,
        Request::BroadcastChange {
            entity: entity("obj-from-b"),
        },
    )
    .await
    .unwrap();
    handle_request(
        &state_a,
        Request::BroadcastChange {
            entity: entity("obj-from-a"),
        },
    )
    .await
    .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let on_a = handle_request(
        &state_a,
        Request::GetObject {
            id: "obj-from-b".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(on_a.is_null(), "pending device must not push data: {on_a}");
    let on_b = handle_request(
        &state_b,
        Request::GetObject {
            id: "obj-from-a".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(
        on_b.is_null(),
        "a pending endpoint must not receive our data: {on_b}"
    );

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}

/// «Отмена» on the initiator drops the dialled connection, which makes the
/// responder's pending request disappear — no orphaned prompts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_pairing_clears_responder_prompt() {
    let (fixture_a, fixture_b, _ticket) = pending_pairing().await;
    let state_a = fixture_a.state.clone();
    let state_b = fixture_b.state.clone();

    handle_request(&state_b, Request::CancelPairing)
        .await
        .unwrap();

    wait_until("A's prompt disappears once B cancels", async || {
        pending_request(&snapshot(&state_a).await, "device-b").is_none()
    })
    .await;
    let snapshot_b = snapshot(&state_b).await;
    assert!(
        snapshot_b["outgoing_pairing"].is_null(),
        "cancel clears the outgoing pairing state: {snapshot_b}"
    );

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}

mod accept_pairing;
mod failure_cases;
mod recovery_tests;
