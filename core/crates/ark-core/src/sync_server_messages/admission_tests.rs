use super::super::*;
use super::revoke_disabled_sessions;
use crate::sync_server::sync_server_tests::tests::MemBackend;

#[tokio::test]
async fn registry_disable_closes_ready_session_and_removes_discovery_record() {
    let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let peer = PeerState {
        device_id: "disabled-device".to_owned(),
        device_name: "Old laptop".to_owned(),
        addresses: Vec::new(),
        platform: None,
        app_version: None,
        authenticated: true,
        sync_complete: true,
        negotiated_protocol: None,
        registry_admission: None,
        in_flight_feed_position: Some(42),
        pending_initial_snapshot_ack: Some("snapshot".to_owned()),
        tx,
    };
    let mut peers = HashMap::new();
    peers.insert(7, peer);
    let peer_record = PeerRecord {
        device_id: "disabled-device".to_owned(),
        device_name: "Old laptop".to_owned(),
        addresses: Vec::new(),
        last_seen: String::new(),
        last_address: None,
        platform: None,
        app_version: None,
    };
    let ctx = MessageContext {
        peers: Arc::new(Mutex::new(peers)),
        storage,
        space_id: Arc::new(RwLock::new("space".to_owned())),
        device_id: Arc::new(RwLock::new("local-device".to_owned())),
        device_name: Arc::new(RwLock::new("Local".to_owned())),
        own_addresses: Arc::new(RwLock::new(Vec::new())),
        auth_secret: Arc::new(RwLock::new(None)),
        known_peer_records: Arc::new(Mutex::new(vec![peer_record])),
        on_change: Arc::new(Mutex::new(None)),
        on_peer_connect: Arc::new(Mutex::new(None)),
        on_new_peer_discovered: Arc::new(Mutex::new(None)),
    };

    assert!(!revoke_disabled_sessions(&ctx, &["disabled-device".to_owned()], None, false).await);
    assert!(ctx.peers.lock().await.is_empty());
    assert!(ctx.known_peer_records.lock().await.is_empty());
    assert!(matches!(rx.recv().await, Some(Message::Close(_))));
}
