use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex};

use crate::hlc::HLC;
use crate::protocol::*;
use crate::relay_transport::{RelayConfig, RelayTransport};
use crate::sync_server::{
    OnChangeCallback, OnPeerConnectCallback, OnPeerDisconnectCallback, StorageBackend,
};
use crate::sync_transport::{SyncTransport, TransportEvent};
use crate::types::{PeerRecord, SyncEntity, VersionVector};

const TAG: &str = "[RelaySync]";
const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
const SYNC_LOAD_PAGE_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct RelaySyncConfig {
    pub relay_url: String,
    pub relay_api_key: Option<String>,
    pub space_id: String,
    pub device_id: String,
    pub device_name: String,
    pub auth_secret: Option<String>,
}

#[derive(Debug, Clone)]
struct RelayPeerState {
    device_name: String,
    authenticated: bool,
}

struct IncomingSyncState {
    vector: VersionVector,
    changed: bool,
    last_update: Instant,
}

pub struct RelaySync {
    storage: Arc<dyn StorageBackend>,
    transport: Arc<dyn SyncTransport>,
    config: RelaySyncConfig,
    auth_secret: Option<String>,
    peers: Arc<Mutex<HashMap<String, RelayPeerState>>>,
    incoming_sync: Arc<Mutex<Option<IncomingSyncState>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
}

impl RelaySync {
    pub fn new(storage: Arc<dyn StorageBackend>, config: RelaySyncConfig) -> Arc<Self> {
        let auth_secret = normalize_auth_secret(config.auth_secret.clone());
        let transport: Arc<dyn SyncTransport> = Arc::new(RelayTransport::new(RelayConfig {
            url: config.relay_url.clone(),
            space_id: config.space_id.clone(),
            device_id: config.device_id.clone(),
            device_name: config.device_name.clone(),
            api_key: config.relay_api_key.clone().unwrap_or_default(),
            auth_secret: auth_secret.clone(),
        }));
        Self::with_transport(storage, config, transport)
    }

    /// Generalized constructor: build `RelaySync` over an already-constructed
    /// transport. Lets callers (e.g. `handle_start_sync`) drive the same CRDT
    /// orchestration with `RelayTransport`, `IrohTransport` (behind
    /// `iroh-spike`), or any other `SyncTransport` impl, instead of always
    /// constructing a `RelayTransport` internally from `config.relay_url`.
    pub fn with_transport(
        storage: Arc<dyn StorageBackend>,
        config: RelaySyncConfig,
        transport: Arc<dyn SyncTransport>,
    ) -> Arc<Self> {
        let auth_secret = normalize_auth_secret(config.auth_secret.clone());

        Arc::new(Self {
            storage,
            transport,
            config,
            auth_secret,
            peers: Arc::new(Mutex::new(HashMap::new())),
            incoming_sync: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_peer_connect: Arc::new(Mutex::new(None)),
            on_peer_disconnect: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn set_on_change(&self, handler: OnChangeCallback) {
        *self.on_change.lock().await = Some(handler);
    }

    pub async fn set_on_peer_connect(&self, handler: OnPeerConnectCallback) {
        *self.on_peer_connect.lock().await = Some(handler);
    }

    pub async fn set_on_peer_disconnect(&self, handler: OnPeerDisconnectCallback) {
        *self.on_peer_disconnect.lock().await = Some(handler);
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), String> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<TransportEvent>();
        self.transport.start(event_tx).await?;

        let this = self.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                this.handle_event(event).await;
            }
        });
        let incoming_sync = self.incoming_sync.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let mut incoming = incoming_sync.lock().await;
                let stale = incoming
                    .as_ref()
                    .is_some_and(|state| state.last_update.elapsed() >= Duration::from_secs(60));
                if stale {
                    *incoming = None;
                    drop(incoming);
                    trim_process_heap();
                }
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        self.transport.stop();
    }

    pub fn broadcast_live_change(&self, entity: SyncEntity) -> Result<(), String> {
        self.transport.send(LanSyncMessage::LiveChange {
            change_id: generate_id(),
            entity,
            origin_device_id: Some(self.config.device_id.clone()),
        })
    }

    pub async fn get_connected_peer_entries(&self) -> Vec<(String, String)> {
        self.peers
            .lock()
            .await
            .iter()
            .filter_map(|(device_id, peer)| {
                if peer.authenticated {
                    Some((device_id.clone(), peer.device_name.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    async fn handle_event(&self, event: TransportEvent) {
        match event {
            TransportEvent::MessageReceived {
                from_device_id,
                msg,
            } => {
                let from = if from_device_id.is_empty() {
                    message_origin_device_id(&msg).unwrap_or_default()
                } else {
                    from_device_id
                };
                if from == self.config.device_id {
                    return;
                }
                self.handle_message(from, msg).await;
            }
            TransportEvent::Connected { .. } => {}
            TransportEvent::Disconnected { device_id } => {
                self.peers.lock().await.remove(&device_id);
                if let Some(handler) = self.on_peer_disconnect.lock().await.as_ref() {
                    handler(device_id, self.peers.lock().await.len());
                }
            }
        }
        trim_process_heap();
    }

    async fn handle_message(&self, from_device_id: String, msg: LanSyncMessage) {
        match msg {
            LanSyncMessage::Hello {
                protocol_version,
                device_id,
                device_name,
                space_id,
                auth_nonce,
                auth_hmac,
                ..
            } => {
                if protocol_version != PROTOCOL_VERSION {
                    eprintln!("{TAG} protocol mismatch from relay peer {device_id}");
                    return;
                }
                if space_id != self.config.space_id {
                    return;
                }
                if device_id == self.config.device_id {
                    return;
                }

                if let Some(secret) = self.auth_secret.as_ref() {
                    let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
                        (Some(nonce), Some(hmac)) => {
                            verify_hello_auth_hmac(secret, &space_id, &device_id, nonce, hmac)
                        }
                        _ => false,
                    };
                    if !valid {
                        eprintln!("{TAG} rejecting relay hello with invalid HMAC");
                        return;
                    }
                }

                let was_new = {
                    let mut peers = self.peers.lock().await;
                    let was_new = !peers.contains_key(&device_id);
                    peers.insert(
                        device_id.clone(),
                        RelayPeerState {
                            device_name: device_name.clone(),
                            authenticated: true,
                        },
                    );
                    was_new
                };

                if was_new {
                    if let Some(handler) = self.on_peer_connect.lock().await.as_ref() {
                        handler(device_id.clone());
                    }
                    let _ = self.transport.send(self.make_hello());
                }

                self.send_local_version_vector().await;
            }

            LanSyncMessage::VersionVector {
                vector: remote_vector,
                origin_device_id,
            } => {
                let origin = origin_device_id.unwrap_or(from_device_id);
                if !self.is_authenticated_peer(&origin).await {
                    return;
                }
                self.send_missing_entities(&remote_vector).await;
            }

            LanSyncMessage::SyncChanges {
                entities,
                is_last,
                origin_device_id,
                ..
            } => {
                let origin = origin_device_id.unwrap_or(from_device_id);
                if !self.is_authenticated_peer(&origin).await {
                    return;
                }
                self.apply_entities(&entities, is_last).await;
            }

            LanSyncMessage::LiveChange {
                entity,
                origin_device_id,
                ..
            } => {
                let origin = origin_device_id.unwrap_or(from_device_id);
                if !self.is_authenticated_peer(&origin).await {
                    return;
                }
                self.apply_entities(&[entity], true).await;
            }

            _ => {}
        }
    }

    async fn is_authenticated_peer(&self, device_id: &str) -> bool {
        self.peers
            .lock()
            .await
            .get(device_id)
            .map(|peer| peer.authenticated)
            .unwrap_or(false)
    }

    fn make_hello(&self) -> LanSyncMessage {
        let (auth_nonce, auth_hmac) = match self.auth_secret.as_ref() {
            Some(secret) => {
                let nonce = generate_auth_nonce();
                let hmac = compute_hello_auth_hmac(
                    secret,
                    &self.config.space_id,
                    &self.config.device_id,
                    &nonce,
                );
                (Some(nonce), Some(hmac))
            }
            None => (None, None),
        };

        LanSyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: self.config.device_id.clone(),
            device_name: self.config.device_name.clone(),
            space_id: self.config.space_id.clone(),
            addresses: None,
            auth_nonce,
            auth_hmac,
        }
    }

    async fn send_local_version_vector(&self) {
        let vector = load_version_vector(&self.storage).await;
        let _ = self.transport.send(LanSyncMessage::VersionVector {
            vector,
            origin_device_id: Some(self.config.device_id.clone()),
        });
    }

    async fn send_missing_entities(&self, remote_vector: &VersionVector) {
        let mut load_vector = load_version_vector(&self.storage).await;
        merge_usage_cursors(&mut load_vector, remote_vector);
        let mut offset = 0;

        loop {
            let entities = self
                .storage
                .load_entities_page(&load_vector, offset, SYNC_LOAD_PAGE_SIZE)
                .await;
            if entities.is_empty() {
                break;
            }
            offset += entities.len();

            let mut to_send = Vec::new();
            for entity in entities {
                if should_send_entity(remote_vector, &entity) {
                    to_send.push(entity);
                }
            }

            for batch in split_into_batches(&to_send) {
                let _ = self.transport.send(LanSyncMessage::SyncChanges {
                    batch_id: generate_id(),
                    entities: batch,
                    is_last: false,
                    origin_device_id: Some(self.config.device_id.clone()),
                });
            }
        }

        let _ = self.transport.send(LanSyncMessage::SyncChanges {
            batch_id: generate_id(),
            entities: vec![],
            is_last: true,
            origin_device_id: Some(self.config.device_id.clone()),
        });
    }

    async fn apply_entities(&self, entities: &[SyncEntity], is_last: bool) {
        let mut incoming = self.incoming_sync.lock().await;
        if incoming.is_none() {
            *incoming = Some(IncomingSyncState {
                vector: load_version_vector(&self.storage).await,
                changed: false,
                last_update: Instant::now(),
            });
        }
        let state = incoming.as_mut().expect("incoming sync state initialized");
        state.last_update = Instant::now();

        for entity in entities {
            let should_apply = is_usage_entity(entity)
                || state
                    .vector
                    .get(&entity.id)
                    .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));
            if !should_apply {
                continue;
            }

            match self.storage.apply_entity(entity).await {
                Ok(()) => {
                    state.changed |= observe_non_usage_entity(&mut state.vector, entity);
                    if let Some(handler) = self.on_change.lock().await.as_ref() {
                        handler(entity.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{TAG} failed to apply relay entity {}:{}: {e}",
                        entity.entity_type, entity.id
                    );
                }
            }
        }

        if is_last {
            if state.changed {
                merge_usage_cursors(&mut state.vector, &load_version_vector(&self.storage).await);
                save_version_vector(&self.storage, &state.vector).await;
            }
            *incoming = None;
            drop(incoming);
            trim_process_heap();
        }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn trim_process_heap() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    // SAFETY: glibc's malloc_trim only asks the allocator to return unused
    // arena pages. It does not invalidate live allocations.
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
