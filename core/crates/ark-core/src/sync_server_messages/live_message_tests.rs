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
        session: test_server_session_handle("peer-a"),
        device_id: "peer-a".to_owned(),
        device_name: "Peer A".to_owned(),
        addresses: Vec::new(),
        platform: None,
        app_version: None,
        outbound_snapshot:
            ark_sync::outbound_snapshot::OutboundSnapshotProgress::awaiting_first_ack(
                ark_sync::outbound_snapshot::PendingSnapshotAck {
                    batch_id: "batch-2".to_owned(),
                    policy_identity: Some(expected.clone()),
                },
            ),
        negotiated_protocol: None,
        in_flight_feed: None,
        tx: PeerMessageSender::test(tx),
    };
    let mut peers = HashMap::new();
    peers.insert(7, peer);
    let ctx = SyncDataPorts {
        peers: Arc::new(Mutex::new(peers)),
        storage: storage_port,
        space_id: Arc::new(RwLock::new("space".to_owned())),
        device_id: Arc::new(RwLock::new("local-device".to_owned())),
        own_addresses: Arc::new(RwLock::new(Vec::new())),
        known_peer_records: Arc::new(Mutex::new(Vec::new())),
        on_change: Arc::new(Mutex::new(None)),
        on_new_peer_discovered: Arc::new(Mutex::new(None)),
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
    assert!(!peer.outbound_snapshot.has_been_acknowledged());
    assert_eq!(
        peer.outbound_snapshot
            .pending_ack()
            .as_ref()
            .map(|pending| pending.batch_id.as_str()),
        Some("batch-2")
    );
    assert_eq!(
        peer.outbound_snapshot
            .pending_ack()
            .as_ref()
            .and_then(|pending| pending.policy_identity.as_ref()),
        Some(&expected)
    );
    assert_eq!(storage.outbound_snapshot_identity_completion_count(), 0);
}
