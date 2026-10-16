use super::*;

#[test]
fn peer_write_gate_allows_only_control_messages_before_admission() {
    let user_data = LanSyncMessage::VersionVector {
        vector: VersionVector::default(),
        origin_device_id: None,
    };
    let registry_control = LanSyncMessage::RegistrySyncStart {
        session_id: "session-1".to_owned(),
    };

    assert!(!peer_message_allowed(false, &user_data));
    assert!(peer_message_allowed(false, &registry_control));
    assert!(peer_message_allowed(true, &user_data));
}

#[tokio::test]
async fn send_signed_integration_frame_requires_addressed_authenticated_peer() {
    let storage =
        Arc::new(sync_server_tests::tests::MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
    let server = SyncServer::new(storage);
    *server.space_id.write().await = "space".to_string();
    *server.device_id.write().await = "me".to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    server.peers.lock().await.insert(
        1,
        PeerState {
            device_id: "peer".to_string(),
            device_name: "Peer".to_string(),
            addresses: vec![],
            platform: None,
            app_version: None,
            authenticated: true,
            sync_complete: true,
            negotiated_protocol: None,
            registry_admission: None,
            in_flight_feed_position: None,
            pending_initial_snapshot_ack: None,
            tx,
        },
    );

    let frame = SignedSyncEnvelope::new("space", "me", "peer", 1, "message", vec![], "sig");
    server
        .send_signed_integration_frame("peer", frame)
        .await
        .expect("addressed authenticated peer should receive the frame");
    assert!(matches!(
        rx.try_recv().expect("frame should be enqueued"),
        Message::Text(_)
    ));

    let wrong_recipient =
        SignedSyncEnvelope::new("space", "me", "other", 1, "message-2", vec![], "sig");
    let error = server
        .send_signed_integration_frame("other", wrong_recipient)
        .await
        .expect_err("unknown recipient must not be routed");
    assert!(error.contains("target LAN peer is not authenticated"));
    assert!(rx.try_recv().is_err());
}
