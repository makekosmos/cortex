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
    use rusqlite::Connection;
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
    fn make_storage(device_id: &str) -> Arc<SqliteStorageBackend> {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ark_store::open::init_schema(&conn).expect("schema");
        let shared = Arc::new(StdMutex::new(conn));
        let backend = Arc::new(SqliteStorageBackend::for_test(shared));
        backend.set_device_id(device_id).unwrap();
        backend
    }
    fn todo_entity(id: &str, title: &str, device_id: &str, counter: u64) -> SyncEntity {
        let mut data = serde_json::Map::new();
        data.insert(
            "title".to_string(),
            serde_json::Value::String(title.to_string()),
        );
        data.insert("notes".to_string(), serde_json::Value::Null);
        data.insert("priority".to_string(), serde_json::json!(0));
        data.insert("scheduledDate".to_string(), serde_json::Value::Null);
        data.insert("deadline".to_string(), serde_json::Value::Null);
        data.insert("reminderDate".to_string(), serde_json::Value::Null);
        data.insert("isToday".to_string(), serde_json::json!(false));
        data.insert("isEvening".to_string(), serde_json::json!(false));
        data.insert("isSomeday".to_string(), serde_json::json!(false));
        data.insert("isCompleted".to_string(), serde_json::json!(false));
        data.insert("completedAt".to_string(), serde_json::Value::Null);
        data.insert("isCancelled".to_string(), serde_json::json!(false));
        data.insert("cancelledAt".to_string(), serde_json::Value::Null);
        data.insert("isTrashed".to_string(), serde_json::json!(false));
        data.insert("sortOrder".to_string(), serde_json::json!(0));
        data.insert("headingId".to_string(), serde_json::Value::Null);
        data.insert("projectId".to_string(), serde_json::Value::Null);
        data.insert("areaId".to_string(), serde_json::Value::Null);
        data.insert("tagIds".to_string(), serde_json::json!([]));
        data.insert("checklistItems".to_string(), serde_json::json!([]));
        data.insert("recurrenceRule".to_string(), serde_json::Value::Null);
        data.insert(
            "createdAt".to_string(),
            serde_json::json!("2026-04-25T00:00:00.000Z"),
        );
        SyncEntity {
            entity_type: "todo".to_string(),
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
        let storage_a = make_storage("device-a");
        let storage_b = make_storage("device-b");
        storage_a
            .apply_entity(&todo_entity("relay-existing", "Existing", "device-a", 1))
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
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_rejects_legacy_peers_before_live_data() {
        let relay_url = test_relay_url().await;
        let storage_a = make_storage("device-a");
        let storage_b = make_storage("device-b");
        let relay_a = relay_sync(storage_a.clone(), &relay_url, "device-a", "Alpha", None);
        relay_a.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let relay_b = relay_sync(storage_b.clone(), &relay_url, "device-b", "Beta", None);
        relay_b.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        let live = todo_entity("relay-live", "Live", "device-a", 2);
        storage_a.apply_entity(&live).await.unwrap();
        let send_error = relay_a.broadcast_live_change(live).await.unwrap_err();
        assert!(send_error.contains("does not support per-device registry admission"));
        tokio::time::sleep(Duration::from_millis(500)).await;
        let loaded_b = storage_b.load_entities(&HashMap::new()).await.unwrap();
        assert!(
            loaded_b.iter().all(|entity| entity.id != "relay-live"),
            "a relay peer without registry admission must not receive live data"
        );
        assert!(
            relay_b.get_connected_peer_entries().await.is_empty(),
            "a peer without registry admission must not become connected"
        );
        relay_a.stop().await;
        relay_b.stop().await;
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_rejects_wrong_hmac_secret() {
        let relay_url = test_relay_url().await;
        let storage_a = make_storage("device-a");
        let storage_b = make_storage("device-b");
        storage_a
            .apply_entity(&todo_entity("relay-denied", "Denied", "device-a", 1))
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
    }

    #[tokio::test]
    async fn relay_payload_sender_cannot_authorize_object_writes() {
        let conn = Connection::open_in_memory().unwrap();
        ark_store::open::init_schema(&conn).unwrap();
        ark_store::sync_kv::set_sync_kv(&conn, "sync.removed_peers", r#"["device-blocked"]"#)
            .unwrap();
        let shared = Arc::new(StdMutex::new(conn));
        let backend = Arc::new(SqliteStorageBackend::for_test(shared.clone()));
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
        let conn = shared.lock().unwrap();
        for candidate in &cases {
            assert!(ark_store::objects::get_object(&conn, &candidate.id)
                .unwrap()
                .is_none());
        }
        let receipts: i64 = conn
            .query_row(
                concat!(
                    "SELECT COUNT(*) FROM entity_write_receipts ",
                    "WHERE entity_id LIKE 'relay-spoofed-object%'"
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipts, 0);
    }
}
