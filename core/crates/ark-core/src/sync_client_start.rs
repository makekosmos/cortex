use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use ark_domain::types::*;
    use ark_sync::protocol::*;
    use futures_util::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    mod registry_admission_tests {
        include!("sync_client_start/registry_admission_tests.rs");
    }

    mod task_merge_tests {
        include!("sync_client_start/task_merge_tests.rs");
    }

    #[tokio::test]
    async fn client_refuses_legacy_peer_before_registry_admission() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let _client_hello = socket.next().await.unwrap().unwrap();
            socket
                .send(Message::Text(
                    serialize_message(&LanSyncMessage::Hello {
                        protocol_version: PROTOCOL_VERSION,
                        device_id: "server-peer".into(),
                        device_name: "Server Peer".into(),
                        space_id: "space-test".into(),
                        addresses: None,
                        auth_nonce: None,
                        auth_hmac: None,
                        platform: None,
                        app_version: None,
                        supported_protocol_versions: None,
                        supported_capabilities: None,
                        negotiated_capabilities: None,
                        negotiation_hmac: None,
                    })
                    .into(),
                ))
                .await
                .unwrap();
            !matches!(
                tokio::time::timeout(std::time::Duration::from_secs(2), socket.next()).await,
                Ok(Some(Ok(Message::Text(text))))
                    if matches!(deserialize_message(&text), Some(
                        LanSyncMessage::VersionVector { .. }
                        | LanSyncMessage::SyncChanges { .. }
                        | LanSyncMessage::LiveChange { .. }
                    ))
            )
        });

        let storage = Arc::new(crate::sync_server::sync_server_tests::tests::MemBackend::new());
        let client = create_direct_client(
            storage as Arc<dyn SyncRuntimeStorage>,
            DirectClientConfig::new(
                PeerRecord {
                    device_id: "server-peer".into(),
                    device_name: "Server Peer".into(),
                    addresses: vec![address.to_string()],
                    last_seen: String::new(),
                    last_address: None,
                    platform: None,
                    app_version: None,
                },
                "client-device".into(),
                "Client Device".into(),
                "space-test".into(),
                vec![],
                None,
            ),
        );
        client.start();
        assert!(
            server.await.unwrap(),
            "a legacy peer must not receive data before registry admission"
        );
        client.stop_and_join().await;
    }

    #[tokio::test]
    async fn client_rejects_tampered_protocol_selection_before_sync() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let connected_from_callback = connected.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let frame = socket.next().await.unwrap().unwrap();
            let Message::Text(frame) = frame else {
                panic!("expected client Hello, got {frame:?}");
            };
            let Some(LanSyncMessage::Hello {
                device_id: _,
                space_id,
                auth_nonce: Some(_),
                protocol_version,
                supported_protocol_versions,
                supported_capabilities,
                ..
            }) = deserialize_message(&frame)
            else {
                panic!("expected authenticated client Hello: {frame}");
            };
            let client_offer = ProtocolOffer::new(
                protocol_version,
                supported_protocol_versions.as_deref(),
                supported_capabilities.as_deref(),
            )
            .unwrap();
            let server_offer = ProtocolOffer::current(true);
            let selection = negotiate_protocol(&client_offer, &server_offer, true).unwrap();
            let server_nonce = generate_auth_nonce();
            let response = LanSyncMessage::Hello {
                protocol_version: selection.version,
                device_id: "server-peer".into(),
                device_name: "Server".into(),
                space_id: space_id.clone(),
                addresses: None,
                auth_nonce: Some(server_nonce.clone()),
                auth_hmac: Some(compute_hello_auth_hmac(
                    "shared-secret",
                    &space_id,
                    "server-peer",
                    &server_nonce,
                )),
                platform: None,
                app_version: None,
                supported_protocol_versions: Some(server_offer.version_ids()),
                supported_capabilities: Some(server_offer.capability_ids()),
                negotiated_capabilities: Some(
                    selection
                        .capabilities
                        .iter()
                        .map(|capability| capability.wire_id().to_owned())
                        .collect(),
                ),
                negotiation_hmac: Some("0".repeat(64)),
            };
            socket
                .send(Message::Text(serialize_message(&response).into()))
                .await
                .unwrap();
            let next = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next()).await;
            assert!(
                !matches!(
                    next,
                    Ok(Some(Ok(Message::Text(text))))
                        if matches!(
                            deserialize_message(&text),
                            Some(LanSyncMessage::VersionVector { .. })
                        )
                ),
                "invalid selection must be rejected before version-vector exchange"
            );
        });

        let storage = Arc::new(crate::sync_server::sync_server_tests::tests::MemBackend::new());
        let client = create_direct_client(
            storage as Arc<dyn SyncRuntimeStorage>,
            DirectClientConfig::new(
                PeerRecord {
                    device_id: "server-peer".into(),
                    device_name: "Server".into(),
                    addresses: vec![address.to_string()],
                    last_seen: String::new(),
                    last_address: None,
                    platform: None,
                    app_version: None,
                },
                "client-device".into(),
                "Client".into(),
                "space-test".into(),
                vec![],
                Some("shared-secret".into()),
            ),
        );
        client
            .set_on_connected(Arc::new(move |_| {
                connected_from_callback.store(true, std::sync::atomic::Ordering::Relaxed);
            }))
            .await;
        client.start();
        server.await.unwrap();
        assert!(!connected.load(std::sync::atomic::Ordering::Relaxed));
        client.stop_and_join().await;
    }
}
