use super::*;

#[path = "sync_client_start/client_hello.rs"]
mod client_hello;
#[path = "sync_client_start/connect_and_receive/feed_recovery.rs"]
mod feed_recovery;
#[path = "sync_client_start/send_missing_entities.rs"]
mod send_missing_entities;
#[path = "sync_client_start/connect_and_receive/sync_changes.rs"]
mod sync_changes;

impl SyncClient {
    /// Start connecting to the peer. Spawns a background task.
    pub fn start(&self) {
        self.stopped
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let storage = self.storage.clone();
        let peer = self.peer.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let space_id = self.space_id.clone();
        let own_addresses = self.own_addresses.clone();
        let auth_secret = self.auth_secret.clone();
        let stopped = self.stopped.clone();
        let authenticated_tx = self.authenticated_tx.clone();
        let feed_in_flight = self.feed_in_flight.clone();
        let on_change = self.on_change.clone();
        let on_connected = self.on_connected.clone();
        let on_disconnected = self.on_disconnected.clone();
        let on_peer_list = self.on_peer_list.clone();
        let writer_tasks = self.tasks.clone();
        let task = tokio::spawn(include!("sync_client_start/part02.rs"));
        track_task(&self.tasks, task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[tokio::test]
    async fn failed_live_entity_apply_does_not_ack() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let storage = Arc::new(crate::sync_server::sync_server_tests::tests::MemBackend::new());
        storage.fail_authenticated_apply("injected write failure");

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
                    })
                    .into(),
                ))
                .await
                .unwrap();
            let _version_vector = socket.next().await.unwrap().unwrap();
            socket
                .send(Message::Text(
                    serialize_message(&LanSyncMessage::LiveChange {
                        change_id: "change-1".into(),
                        entity: SyncEntity {
                            entity_type: "todo".into(),
                            id: "todo-1".into(),
                            data: serde_json::Map::new(),
                            hlc: "2026-10-16T00:00:00.000Z-0000-test".into(),
                            deleted: None,
                            origin_device_id: None,
                            origin_seq: None,
                        },
                        origin_device_id: None,
                    })
                    .into(),
                ))
                .await
                .unwrap();

            !matches!(
                tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
                    .await
                    .expect("client should close after apply failure"),
                Some(Ok(Message::Text(text)))
                    if matches!(deserialize_message(&text), Some(LanSyncMessage::LiveAck { .. }))
            )
        });

        let client = SyncClient::new(
            storage as Arc<dyn SyncRuntimeStorage>,
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
        );
        client.start();
        assert!(server.await.unwrap(), "failed apply must not emit LiveAck");
        client.stop_and_join().await;
    }
}
