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
async fn load_version_vector(storage: &Arc<dyn StorageBackend>) -> VersionVector {
    match storage.get_kv(VERSION_VECTOR_KEY).await {
        Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        None => VersionVector::new(),
    }
}
async fn save_version_vector(storage: &Arc<dyn StorageBackend>, vector: &VersionVector) {
    let json = serde_json::to_string(vector).unwrap_or_default();
    storage.set_kv(VERSION_VECTOR_KEY, &json).await;
}
#[allow(dead_code)]
fn _touch_peer_record(_: Option<PeerRecord>) {}
#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;
    use crate::db::{init_schema, SqliteStorageBackend};
    type Room = HashMap<String, mpsc::UnboundedSender<Message>>;
    type Rooms = Arc<StdMutex<HashMap<String, Room>>>;
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
                                Some(Ok(msg @ Message::Text(_))) | Some(Ok(msg @ Message::Binary(_))) => {
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
        init_schema(&conn).expect("schema");
        let shared = Arc::new(StdMutex::new(conn));
        let backend = Arc::new(SqliteStorageBackend::new(shared));
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
            storage as Arc<dyn StorageBackend>,
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
    async fn relay_syncs_existing_entity_between_two_runtimes() {
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
        let loaded_b = storage_b.load_entities(&HashMap::new()).await;
        assert!(
            loaded_b.iter().any(|entity| entity.id == "relay-existing"),
            "relay peer should receive existing entity; got {:?}",
            loaded_b.iter().map(|entity| &entity.id).collect::<Vec<_>>()
        );
        relay_a.stop();
        relay_b.stop();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn relay_propagates_live_change() {
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
        relay_a.broadcast_live_change(live).unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        let loaded_b = storage_b.load_entities(&HashMap::new()).await;
        assert!(
            loaded_b.iter().any(|entity| entity.id == "relay-live"),
            "relay peer should receive live entity"
        );
        relay_a.stop();
        relay_b.stop();
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
        let loaded_b = storage_b.load_entities(&HashMap::new()).await;
        assert!(
            !loaded_b.iter().any(|entity| entity.id == "relay-denied"),
            "wrong-secret relay peer must not receive existing entity"
        );
        relay_a.stop();
        relay_b.stop();
    }
}
