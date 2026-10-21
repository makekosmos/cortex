use super::*;

#[tokio::test]
async fn send_signed_integration_frame_requires_addressed_authenticated_peer() {
    let storage =
        Arc::new(sync_server_tests::tests::MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
    let server = SyncServer::new(storage);
    *server.space_id.write().await = "space".to_string();
    *server.device_id.write().await = "me".to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let (session, _keepalive) = test_server_session_with_keepalive("peer");
    server.peers.lock().await.insert(
        1,
        PeerState {
            session,
            tx: PeerMessageSender::test(tx),
        },
    );

    let frame = SignedSyncEnvelope::new("space", "me", "peer", 1, "message", vec![], "sig");
    server
        .send_signed_integration_frame("peer", frame)
        .await
        .expect("addressed authenticated peer should accept the frame intent");
    assert!(rx.try_recv().is_err());

    let wrong_recipient =
        SignedSyncEnvelope::new("space", "me", "other", 1, "message-2", vec![], "sig");
    let error = server
        .send_signed_integration_frame("other", wrong_recipient)
        .await
        .expect_err("unknown recipient must not be routed");
    assert!(error.contains("target LAN peer is not authenticated"));
    assert!(rx.try_recv().is_err());
}
