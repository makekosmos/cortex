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
            platform: None,
            app_version: None,
        }
    }

    /// A client asleep in its reconnect backoff holds `storage` (the db
    /// connection). `stop_and_join` must end that task, so the storage is
    /// released by the time it returns, not whenever the backoff ends.
    #[tokio::test]
    async fn client_stop_and_join_releases_storage() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
        let client = crate::sync_client::SyncClient::new(
            storage.clone(),
            peer_rec("peer-dead", &["127.0.0.1:1"]),
            "me".to_string(),
            "me-name".to_string(),
            "space".to_string(),
            Vec::new(),
            None,
        );
        client.start();
        // The address refuses at once; the task then sleeps in its backoff.
        tokio::time::sleep(Duration::from_millis(100)).await;
        client.stop_and_join().await;
        drop(client);
        assert_eq!(
            Arc::strong_count(&storage),
            1,
            "client task still holds storage after stop_and_join"
        );
    }

    #[tokio::test]
    async fn stop_releases_background_tasks_holding_peers_map() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
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
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        server.block_peer("blocked").await.unwrap();

        // Authenticated session that delivers a PeerList.
        let (tx, _rx) = mpsc::unbounded_channel::<Message>();
        server.peers.lock().await.insert(
            7,
            PeerState {
                device_id: "gossip".to_string(),
                device_name: "Gossip".to_string(),
                addresses: vec![],
                platform: None,
                app_version: None,
                ready_peer: ReadyPeer::new(7, "gossip".into(), "test-session".into()),
                outbound_snapshot:
                    ark_sync::outbound_snapshot::OutboundSnapshotProgress::acknowledged(),
                negotiated_protocol: None,
                registry_admission: None,
                in_flight_feed: None,
                tx: PeerMessageSender::test(tx),
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
            task_merge_bootstrap_v2: server.task_merge_bootstrap_v2,
            known_peer_records: server.known_peer_records.clone(),
            on_change: server.on_change.clone(),
            on_peer_connect: server.on_peer_connect.clone(),
            on_new_peer_discovered: server.on_new_peer_discovered.clone(),
            on_registry_events: server.on_registry_events.clone(),
        };
        sync_server_inbound::handle_peer_list(
            &ctx,
            7,
            vec![
                peer_rec("blocked", &["192.168.1.30:21531"]),
                peer_rec("fresh", &["192.168.1.31:21531"]),
            ],
        )
        .await;

        let ids: Vec<String> = server
            .get_known_peers()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.device_id)
            .collect();
        assert!(ids.contains(&"fresh".to_string()));
        assert!(
            !ids.contains(&"blocked".to_string()),
            "PeerList resurrected a blocked peer: {ids:?}"
        );
        let persisted = storage
            .load_known_peers()
            .await
            .unwrap()
            .unwrap_or_default();
        assert!(
            !persisted.iter().any(|peer| peer.device_id == "blocked"),
            "blocked peer persisted back into known peers: {persisted:?}"
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
            platform: None,
            app_version: None,
            supported_protocol_versions: None,
            supported_capabilities: None,
            negotiated_capabilities: None,
            negotiation_hmac: None,
        }
    }

    /// A rejected hello must close the socket — otherwise the dialer waits
    /// forever on a half-open connection and never retries.
    async fn assert_hello_rejected_with_close(hello: LanSyncMessage, port: u16) {
        let expected_target = match &hello {
            LanSyncMessage::Hello { device_id, .. } => device_id.as_str(),
            _ => panic!("test helper only accepts Hello"),
        };
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        ws.send(Message::Text(serialize_message(&hello).into()))
            .await
            .expect("send hello");
        let frame = tokio::time::timeout(Duration::from_secs(2), ws.next()).await;
        match frame {
            Ok(Some(Ok(Message::Close(_)))) => {}
            Ok(Some(Ok(Message::Text(text)))) => {
                assert!(
                    matches!(
                        deserialize_message(&text),
                        Some(LanSyncMessage::ProtocolRejected { target_device_id, .. })
                            if target_device_id == expected_target
                    ),
                    "unexpected rejection response: {text}"
                );
                let close = tokio::time::timeout(Duration::from_secs(2), ws.next()).await;
                assert!(
                    matches!(close, Ok(Some(Ok(Message::Close(_))))),
                    "expected Close after ProtocolRejected, got {close:?}"
                );
            }
            other => panic!("expected Close or targeted ProtocolRejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejected_hellos_close_connection_and_evict_session() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
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

        server.block_peer("blocked-remote").await.unwrap();
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

    #[tokio::test]
    async fn authenticated_hello_negotiates_receipt_ack_with_a_bound_selection() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
        let server = SyncServer::new(storage);
        server.set_auth_secret(Some("shared-secret".into())).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        server
            .start_with_addr(
                "space-x",
                "server-device",
                Some("Server"),
                None,
                &format!("127.0.0.1:{port}"),
            )
            .await
            .unwrap();

        let client_device_id = "client-device";
        let nonce = generate_auth_nonce();
        let offer = ProtocolOffer::current_with_registry_history(true);
        let offer_hmac = compute_negotiation_offer_hmac(
            "shared-secret",
            "space-x",
            client_device_id,
            &nonce,
            &offer,
        );
        let hello = LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: client_device_id.to_owned(),
            device_name: "Client".into(),
            space_id: "space-x".into(),
            addresses: None,
            auth_nonce: Some(nonce.clone()),
            auth_hmac: Some(compute_hello_auth_hmac(
                "shared-secret",
                "space-x",
                client_device_id,
                &nonce,
            )),
            platform: None,
            app_version: None,
            supported_protocol_versions: Some(offer.version_ids()),
            supported_capabilities: Some(offer.capability_ids()),
            negotiated_capabilities: None,
            negotiation_hmac: Some(offer_hmac),
        };
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        ws.send(Message::Text(serialize_message(&hello).into()))
            .await
            .expect("send hello");
        let response = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("hello response timeout")
            .expect("server closed before response")
            .expect("read response");
        let Message::Text(response) = response else {
            panic!("expected Hello response, got {response:?}");
        };
        let Some(LanSyncMessage::Hello {
            protocol_version,
            device_id,
            space_id,
            auth_nonce,
            auth_hmac,
            supported_protocol_versions,
            supported_capabilities,
            negotiated_capabilities,
            negotiation_hmac,
            ..
        }) = deserialize_message(&response)
        else {
            panic!("expected negotiated Hello, got {response}");
        };
        assert!(verify_hello_auth_hmac(
            "shared-secret",
            &space_id,
            &device_id,
            auth_nonce.as_deref().unwrap(),
            auth_hmac.as_deref().unwrap(),
        ));
        let negotiated = validate_protocol_selection(
            &offer,
            protocol_version,
            supported_protocol_versions.as_deref(),
            supported_capabilities.as_deref(),
            negotiated_capabilities.as_deref(),
            Some("shared-secret"),
            "space-x",
            client_device_id,
            &device_id,
            Some(&nonce),
            auth_nonce.as_deref(),
            negotiation_hmac.as_deref(),
        )
        .expect("selection must be proven against both offers");
        assert!(negotiated
            .capabilities
            .contains(&ProtocolCapability::DurableReceiptAckV1));
        ws.close(None).await.unwrap();
        server.stop().await;
    }

    #[tokio::test]
    async fn invalid_offer_proof_is_refused_before_peer_admission() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
        let server = SyncServer::new(storage);
        server.set_auth_secret(Some("shared-secret".into())).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        server
            .start_with_addr(
                "space-x",
                "server-device",
                Some("Server"),
                None,
                &format!("127.0.0.1:{port}"),
            )
            .await
            .unwrap();

        let device_id = "bad-proof-peer";
        let nonce = generate_auth_nonce();
        let offer = ProtocolOffer::current(true);
        let hello = LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: device_id.into(),
            device_name: "Bad proof".into(),
            space_id: "space-x".into(),
            addresses: None,
            auth_nonce: Some(nonce.clone()),
            auth_hmac: Some(compute_hello_auth_hmac(
                "shared-secret",
                "space-x",
                device_id,
                &nonce,
            )),
            platform: None,
            app_version: None,
            supported_protocol_versions: Some(offer.version_ids()),
            supported_capabilities: Some(offer.capability_ids()),
            negotiated_capabilities: None,
            negotiation_hmac: Some("0".repeat(64)),
        };
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        ws.send(Message::Text(serialize_message(&hello).into()))
            .await
            .expect("send hello");
        let response = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("rejection timeout")
            .expect("server closed before rejection")
            .expect("read rejection");
        let Message::Text(response) = response else {
            panic!("expected targeted protocol refusal, got {response:?}");
        };
        let refusal = deserialize_message(&response);
        assert!(
            matches!(
                &refusal,
                Some(LanSyncMessage::ProtocolRejected {
                    ref target_device_id,
                    ref reason,
                    ..
                }) if target_device_id == device_id && reason == "invalid_protocol_selection_proof"
            ),
            "unexpected protocol refusal: {refusal:?}"
        );
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), ws.next()).await,
            Ok(Some(Ok(Message::Close(_))))
        ));
        assert!(server.peers.lock().await.is_empty());
        assert!(server.get_known_peers().await.unwrap().is_empty());
        server.stop().await;
    }
}
