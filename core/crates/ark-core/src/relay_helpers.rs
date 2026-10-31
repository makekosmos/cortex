#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn trim_process_heap() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    unsafe {
        let _ = malloc_trim(0);
    }
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn trim_process_heap() {}
async fn load_version_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<VersionVector, String> {
    storage.load_version_vector().await
}

async fn apply_relay_entity(
    storage: &Arc<dyn SyncRuntimeStorage>,
    entity: &SyncEntity,
    authenticated_peer_id: Option<&str>,
) -> Result<(), String> {
    // Relay WebSocket frames have no authenticated sender envelope. Object
    // type and deletion state may be stored outside the payload, so every
    // object mutation requires a transport-bound peer identity.
    if entity.entity_type == "object" {
        let peer_id = authenticated_peer_id
            .ok_or_else(|| "authenticated_transport_identity_unavailable".to_string())?;
        return storage.apply_authenticated_entity(entity, peer_id).await;
    }
    match authenticated_peer_id {
        Some(peer_id) => storage.apply_authenticated_entity(entity, peer_id).await,
        None => storage.apply_entity(entity).await,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SqliteStorageBackend;
    use crate::sync_server::sync_server_tests::tests::MemBackend;
    use crate::sync_transport::{SyncTransport, TransportEvent};
    use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};
    use futures_util::{SinkExt, StreamExt};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex,
    };
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;
    type Room = HashMap<String, mpsc::UnboundedSender<Message>>;
    type Rooms = Arc<StdMutex<HashMap<String, Room>>>;

    struct TrackingTransport;

    #[async_trait::async_trait]
    impl SyncTransport for TrackingTransport {
        async fn start(&self, _: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String> {
            Ok(())
        }

        fn send(&self, _: LanSyncMessage) -> Result<(), String> {
            Ok(())
        }

        async fn stop(&self) {}
    }

    #[tokio::test]
    async fn disconnect_evicts_live_peer_even_when_pairing_key_read_fails() {
        let backend = Arc::new(MemBackend::new());
        backend.fail_peer_transport_keys_read("injected database read failure");
        let storage = backend as Arc<dyn SyncRuntimeStorage>;
        let transport = Arc::new(TrackingTransport);
        let relay = RelaySync::with_transport(
            storage,
            RelaySyncConfig {
                relay_url: String::new(),
                relay_api_key: None,
                space_id: "space".into(),
                device_id: "local".into(),
                device_name: "Local".into(),
                auth_secret: None,
            },
            transport.clone(),
        );
        relay.peers.lock().await.insert(
            "peer".into(),
            RelayPeerState {
                device_name: "Peer".into(),
                platform: None,
                app_version: None,
                authenticated: true,
                negotiated_protocol: None,
            },
        );
        let disconnect_notified = Arc::new(AtomicBool::new(false));
        let notified = disconnect_notified.clone();
        relay
            .set_on_peer_disconnect(Arc::new(move |_, _| {
                notified.store(true, Ordering::Relaxed);
            }))
            .await;

        let error = relay
            .disconnect_peer("peer")
            .await
            .expect_err("persistence failure must reach the caller");
        assert!(error.contains("injected database read failure"));
        assert!(!relay.peers.lock().await.contains_key("peer"));
        assert!(disconnect_notified.load(Ordering::Relaxed));
    }

    // The accept_hdr_async callback must return tungstenite's
    // `Result<Response, ErrorResponse>` — Err is the HTTP error response
    // itself, which the lint counts as oversized. Boxing is not an option:
    // the signature is fixed by the library.
    #[allow(clippy::result_large_err)]
    async fn start_test_relay(rooms: Rooms, listener: TcpListener) {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let rooms = rooms.clone();
            tokio::spawn(async move {
                let mut space_id = String::new();
                let mut device_id = String::new();
                let ws_stream = tokio_tungstenite::accept_hdr_async(
                    stream,
                    |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                     resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
                        let query = req.uri().query().unwrap_or("");
                        for pair in query.split('&') {
                            if let Some((k, v)) = pair.split_once('=') {
                                match k {
                                    "space_id" => space_id = v.to_string(),
                                    "device_id" => device_id = v.to_string(),
                                    _ => {}
                                }
                            }
                        }
                        Ok(resp)
                    },
                )
                .await
                .unwrap();
                let (mut ws_tx, mut ws_rx) = ws_stream.split();
                let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
                {
                    let mut guard = rooms.lock().unwrap();
                    guard
                        .entry(space_id.clone())
                        .or_default()
                        .insert(device_id.clone(), out_tx);
                }
                let mut write_task = tokio::spawn(async move {
                    while let Some(msg) = out_rx.recv().await {
                        if ws_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                });
                loop {
                    tokio::select! {
                        _ = &mut write_task => break,
                        frame = ws_rx.next() => {
                            match frame {
                                Some(
                                    Ok(msg @ Message::Text(_))
                                ) | Some(Ok(msg @ Message::Binary(_))) => {
                                    let guard = rooms.lock().unwrap();
                                    if let Some(room) = guard.get(&space_id) {
                                        for (dev, tx) in room {
                                            if dev != &device_id {
                                                let _ = tx.send(msg.clone());
                                            }
                                        }
                                    }
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    let _ = data;
                                }
                                _ => break,
                            }
                        }
                    }
                }
                write_task.abort();
                if let Some(room) = rooms.lock().unwrap().get_mut(&space_id) {
                    room.remove(&device_id);
                }
            });
        }
    }
    async fn test_relay_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let rooms: Rooms = Arc::new(StdMutex::new(HashMap::new()));
        tokio::spawn(start_test_relay(rooms, listener));
        tokio::time::sleep(Duration::from_millis(50)).await;
        format!("ws://127.0.0.1:{port}")
    }
    async fn make_storage(
        device_id: &str,
    ) -> (
        Arc<SqliteStorageBackend>,
        ark_store::test_support::TestDatabase,
    ) {
        let database = ark_store::test_support::TestDatabase::create().expect("test database");
        let backend = Arc::new(
            SqliteStorageBackend::open_for_test(
                database.path().to_string_lossy().into_owned(),
                device_id,
            )
            .await
            .expect("open test storage"),
        );
        backend.set_device_id(device_id).unwrap();
        (backend, database)
    }
    fn core_note_entity(id: &str, title: &str, device_id: &str, counter: u64) -> SyncEntity {
        let data: serde_json::Value = serde_json::from_value(serde_json::json!({
            "typeId": "core.note",
            "typeVersion": "0.1.0",
            "title": title,
            "contentJson": {"type":"doc", "content":[{"type":"paragraph"}]},
            "propsJson": {"description":null, "extensions":{}},
            "createdAt": "2026-04-25T00:00:00.000Z",
            "updatedAt": "2026-04-25T00:00:00.000Z",
            "deletedAt": null
        }))
        .expect("core.note sync payload is valid");
        let data = data
            .as_object()
            .cloned()
            .expect("core.note sync payload is an object");
        SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data,
            hlc: format!("2026-04-25T00:00:00.000Z:{counter:06}:{device_id}"),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }
    fn relay_sync(
        storage: Arc<SqliteStorageBackend>,
        relay_url: &str,
        device_id: &str,
        device_name: &str,
        auth_secret: Option<&str>,
    ) -> Arc<RelaySync> {
        RelaySync::new(
            storage as Arc<dyn SyncRuntimeStorage>,
            RelaySyncConfig {
                relay_url: relay_url.to_string(),
                relay_api_key: None,
                space_id: "relay-space".to_string(),
                device_id: device_id.to_string(),
                device_name: device_name.to_string(),
                auth_secret: auth_secret.map(str::to_string),
            },
        )
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_websocket_fails_closed_for_existing_core_task_without_bound_identity() {
        let relay_url = test_relay_url().await;
        let (storage_a, _database_a) = make_storage("device-a").await;
        let (storage_b, _database_b) = make_storage("device-b").await;
        storage_a
            .apply_entity(&core_note_entity(
                "relay-existing",
                "Existing",
                "device-a",
                1,
            ))
            .await
            .unwrap();
        let relay_a = relay_sync(
            storage_a.clone(),
            &relay_url,
            "device-a",
            "Alpha",
            Some("secret"),
        );
        relay_a.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let relay_b = relay_sync(
            storage_b.clone(),
            &relay_url,
            "device-b",
            "Beta",
            Some("secret"),
        );
        relay_b.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(700)).await;
        let loaded_b = storage_b.load_entities(&HashMap::new()).await.unwrap();
        assert!(
            loaded_b.iter().all(|entity| entity.id != "relay-existing"),
            "unauthenticated WebSocket must not apply a core.task payload"
        );
        relay_a.stop().await;
        relay_b.stop().await;
        storage_a
            .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
            .await
            .unwrap();
        storage_b
            .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
            .await
            .unwrap();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_rejects_wrong_hmac_secret() {
        let relay_url = test_relay_url().await;
        let (storage_a, _database_a) = make_storage("device-a").await;
        let (storage_b, _database_b) = make_storage("device-b").await;
        storage_a
            .apply_entity(&core_note_entity("relay-denied", "Denied", "device-a", 1))
            .await
            .unwrap();
        let relay_a = relay_sync(
            storage_a.clone(),
            &relay_url,
            "device-a",
            "Alpha",
            Some("correct"),
        );
        relay_a.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let relay_b = relay_sync(
            storage_b.clone(),
            &relay_url,
            "device-b",
            "Beta",
            Some("wrong"),
        );
        relay_b.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(
            relay_a.get_connected_peer_entries().await.is_empty(),
            "wrong-secret relay peer must not authenticate"
        );
        let loaded_b = storage_b.load_entities(&HashMap::new()).await.unwrap();
        assert!(
            !loaded_b.iter().any(|entity| entity.id == "relay-denied"),
            "wrong-secret relay peer must not receive existing entity"
        );
        relay_a.stop().await;
        relay_b.stop().await;
        storage_a
            .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
            .await
            .unwrap();
        storage_b
            .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn relay_payload_sender_cannot_authorize_object_writes() {
        let database = ark_store::test_support::TestDatabase::create().unwrap();
        ark_store::test_support::SyncTestState::add_removed_peer(database.path(), "device-blocked")
            .unwrap();
        let backend = Arc::new(
            SqliteStorageBackend::open_for_test(
                database.path().to_string_lossy().into_owned(),
                "device-local",
            )
            .await
            .unwrap(),
        );
        let storage = backend.clone() as Arc<dyn SyncRuntimeStorage>;
        let entity = SyncEntity {
            entity_type: "object".into(),
            id: "relay-spoofed-task".into(),
            data: serde_json::json!({
                "typeId": "core.task",
                "typeVersion": "0.1.0",
                "title": "Spoofed",
                "createdAt": "2026-10-01T00:00:00Z",
                "updatedAt": "2026-10-01T00:00:00Z",
                "deletedAt": null,
                "contentJson": {},
                "propsJson": {}
            })
            .as_object()
            .unwrap()
            .clone(),
            hlc: "2026-10-15T00:00:00.000Z:000001:device-blocked".into(),
            deleted: None,
            origin_device_id: Some("device-blocked".into()),
            origin_seq: None,
        };

        let mut cases = vec![entity.clone()];
        let mut missing_type = entity.clone();
        missing_type.id = "relay-spoofed-object-missing-type".into();
        missing_type.data.remove("typeId");
        cases.push(missing_type);
        let mut malformed_type = entity.clone();
        malformed_type.id = "relay-spoofed-object-malformed-type".into();
        malformed_type
            .data
            .insert("typeId".into(), serde_json::json!(null));
        cases.push(malformed_type);
        let mut tombstone = entity;
        tombstone.id = "relay-spoofed-object-tombstone".into();
        tombstone.data.clear();
        tombstone.deleted = Some(true);
        cases.push(tombstone);

        for candidate in &cases {
            assert_eq!(
                apply_relay_entity(&storage, candidate, None)
                    .await
                    .unwrap_err(),
                "authenticated_transport_identity_unavailable"
            );
        }
        let persisted = storage.load_entities(&HashMap::new()).await.unwrap();
        for candidate in &cases {
            assert!(!persisted.iter().any(|entity| entity.id == candidate.id));
        }
        backend
            .shutdown_for_test(std::time::Instant::now() + Duration::from_secs(5))
            .await
            .unwrap();
    }
}
