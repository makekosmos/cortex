use super::*;

#[cfg(test)]
pub(super) mod reject_tests {
    use super::super::sync_server_tests::tests::MemBackend;
    use super::*;
    use futures_util::{SinkExt, StreamExt};

    fn peer_rec(device_id: &str, addrs: &[&str]) -> PeerRecord {
        PeerRecord {
            device_id: device_id.to_string(),
            device_name: format!("{device_id}-name"),
            addresses: addrs.iter().map(|s| s.to_string()).collect(),
            last_seen: "2026-04-01T00:00:00.000Z".to_string(),
            last_address: None,
        }
    }

    #[tokio::test]
    async fn stop_releases_background_tasks_holding_peers_map() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage);

        // Start + stop twice: a leaked ping task survives each start and
        // would pin another `peers` Arc clone every cycle.
        for _ in 0..2 {
            server
                .start_with_addr("space", "me", None, None, "127.0.0.1:0")
                .await
                .unwrap();
            server.stop().await;
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while Arc::strong_count(&server.peers) > 1 {
            assert!(
                std::time::Instant::now() < deadline,
                "leaked background task still holds the peers map \
                 (strong_count = {})",
                Arc::strong_count(&server.peers)
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn peer_list_ignores_blocked_peers() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        server.block_peer("blocked").await;

        // Authenticated session that delivers a PeerList.
        let (tx, _rx) = mpsc::unbounded_channel::<Message>();
        server.peers.lock().await.insert(
            7,
            PeerState {
                device_id: "gossip".to_string(),
                device_name: "Gossip".to_string(),
                addresses: vec![],
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx,
            },
        );

        let ctx = MessageContext {
            peers: server.peers.clone(),
            storage: storage.clone(),
            space_id: server.space_id.clone(),
            device_id: server.device_id.clone(),
            device_name: server.device_name.clone(),
            own_addresses: server.own_addresses.clone(),
            auth_secret: server.auth_secret.clone(),
            known_peer_records: server.known_peer_records.clone(),
            on_change: server.on_change.clone(),
            on_peer_connect: server.on_peer_connect.clone(),
            on_new_peer_discovered: server.on_new_peer_discovered.clone(),
        };
        sync_server_messages::handle_message(
            &ctx,
            7,
            LanSyncMessage::PeerList {
                peers: vec![
                    peer_rec("blocked", &["192.168.1.30:21531"]),
                    peer_rec("fresh", &["192.168.1.31:21531"]),
                ],
            },
        )
        .await;

        let ids: Vec<String> = server
            .get_known_peers()
            .await
            .into_iter()
            .map(|p| p.device_id)
            .collect();
        assert!(ids.contains(&"fresh".to_string()));
        assert!(
            !ids.contains(&"blocked".to_string()),
            "PeerList resurrected a blocked peer: {ids:?}"
        );
        let persisted = storage.get_kv(KNOWN_PEERS_KEY).await.unwrap_or_default();
        assert!(
            !persisted.contains("blocked"),
            "blocked peer persisted back into {KNOWN_PEERS_KEY}: {persisted}"
        );
    }

    fn hello_msg(device_id: &str, space_id: &str, protocol_version: u32) -> LanSyncMessage {
        LanSyncMessage::Hello {
            protocol_version,
            device_id: device_id.to_string(),
            device_name: format!("{device_id}-name"),
            space_id: space_id.to_string(),
            addresses: None,
            auth_nonce: None,
            auth_hmac: None,
        }
    }

    /// A rejected hello must close the socket — otherwise the dialer waits
    /// forever on a half-open connection and never retries.
    async fn assert_hello_rejected_with_close(hello: LanSyncMessage, port: u16) {
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        ws.send(Message::Text(serialize_message(&hello)))
            .await
            .expect("send hello");
        let frame = tokio::time::timeout(Duration::from_secs(2), ws.next()).await;
        assert!(
            matches!(frame, Ok(Some(Ok(Message::Close(_))))),
            "expected Close frame for rejected hello, got {frame:?}"
        );
    }

    #[tokio::test]
    async fn rejected_hellos_close_connection_and_evict_session() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        server
            .start_with_addr(
                "space-x",
                "me",
                Some("Me"),
                None,
                &format!("127.0.0.1:{port}"),
            )
            .await
            .unwrap();

        assert_hello_rejected_with_close(hello_msg("me", "space-x", PROTOCOL_VERSION), port).await;
        assert_hello_rejected_with_close(
            hello_msg("other", "space-x", PROTOCOL_VERSION + 99),
            port,
        )
        .await;
        assert_hello_rejected_with_close(hello_msg("other", "space-y", PROTOCOL_VERSION), port)
            .await;

        server.block_peer("blocked-remote").await;
        assert_hello_rejected_with_close(
            hello_msg("blocked-remote", "space-x", PROTOCOL_VERSION),
            port,
        )
        .await;

        assert!(
            server.peers.lock().await.is_empty(),
            "rejected sessions must not linger in the peers map"
        );
        server.stop().await;
    }
}
