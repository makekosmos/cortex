use super::super::*;
use super::handle_live_message;
use crate::sync_server::sync_server_tests::tests::MemBackend;
use ark_application::sync_storage::PolicyIdentity;

#[tokio::test]
async fn stale_snapshot_ack_does_not_complete_or_clear_pending_snapshot() {
    let storage = Arc::new(MemBackend::new());
    let storage_port = storage.clone() as Arc<dyn SyncRuntimeStorage>;
    let expected = PolicyIdentity::new(2, "a".repeat(64), true).unwrap();
    let stale = PolicyIdentity::new(1, "b".repeat(64), true).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let peer = PeerState {
        device_id: "peer-a".to_owned(),
        device_name: "Peer A".to_owned(),
        addresses: Vec::new(),
        platform: None,
        app_version: None,
        ready_peer: ReadyPeer::new(7, "peer-a".into(), "session-a".into()),
        sync_started: false,
        sync_complete: false,
        negotiated_protocol: None,
        registry_admission: None,
        in_flight_feed_position: None,
        in_flight_feed_policy_identity: None,
        pending_initial_snapshot_ack: Some("batch-2".to_owned()),
        pending_initial_snapshot_policy_identity: Some(expected.clone()),
        tx,
    };
    let mut peers = HashMap::new();
    peers.insert(7, peer);
    let ctx = MessageContext {
        peers: Arc::new(Mutex::new(peers)),
        storage: storage_port,
        space_id: Arc::new(RwLock::new("space".to_owned())),
        device_id: Arc::new(RwLock::new("local-device".to_owned())),
        device_name: Arc::new(RwLock::new("Local".to_owned())),
        own_addresses: Arc::new(RwLock::new(Vec::new())),
        auth_secret: Arc::new(RwLock::new(None)),
        task_merge_bootstrap_v2: false,
        known_peer_records: Arc::new(Mutex::new(Vec::new())),
        on_change: Arc::new(Mutex::new(None)),
        on_peer_connect: Arc::new(Mutex::new(None)),
        on_new_peer_discovered: Arc::new(Mutex::new(None)),
        on_registry_events: Arc::new(Mutex::new(None)),
    };

    handle_live_message(
        &ctx,
        7,
        LanSyncMessage::SyncAck {
            batch_id: "batch-2".to_owned(),
            accepted: 0,
            policy_identity: Some(stale),
        },
    )
    .await;

    let peers = ctx.peers.lock().await;
    let peer = peers.get(&7).unwrap();
    assert!(!peer.sync_complete);
    assert_eq!(
        peer.pending_initial_snapshot_ack.as_deref(),
        Some("batch-2")
    );
    assert_eq!(
        peer.pending_initial_snapshot_policy_identity.as_ref(),
        Some(&expected)
    );
    assert_eq!(storage.outbound_snapshot_identity_completion_count(), 0);
}
