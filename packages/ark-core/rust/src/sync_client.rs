use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::hlc::HLC;
use crate::protocol::*;
use crate::sync_server::StorageBackend;
use crate::types::*;

const TAG: &str = "[SyncClient]";
const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
const CONNECT_TIMEOUT_MS: u64 = 5_000;
const RECONNECT_BASE_MS: u64 = 2_000;
const RECONNECT_MAX_MS: u64 = 30_000;

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

pub type ClientOnChangeCallback = Arc<dyn Fn(SyncEntity) + Send + Sync>;
pub type ClientOnConnectedCallback = Arc<dyn Fn(String, String) + Send + Sync>;
pub type ClientOnDisconnectedCallback = Arc<dyn Fn(String) + Send + Sync>;
pub type ClientOnPeerListCallback = Arc<dyn Fn(Vec<PeerRecord>) + Send + Sync>;

// ---------------------------------------------------------------------------
// SyncClient
// ---------------------------------------------------------------------------

pub struct SyncClient {
    storage: Arc<dyn StorageBackend>,
    peer: Arc<RwLock<PeerRecord>>,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
    authenticated_tx: Arc<Mutex<Option<mpsc::UnboundedSender<Message>>>>,
    on_change: Arc<Mutex<Option<ClientOnChangeCallback>>>,
    on_connected: Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    on_disconnected: Arc<Mutex<Option<ClientOnDisconnectedCallback>>>,
    on_peer_list: Arc<Mutex<Option<ClientOnPeerListCallback>>>,
}

impl SyncClient {
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        peer: PeerRecord,
        device_id: String,
        device_name: String,
        space_id: String,
        own_addresses: Vec<String>,
    ) -> Self {
        Self {
            storage,
            peer: Arc::new(RwLock::new(peer)),
            device_id,
            device_name,
            space_id,
            own_addresses,
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            authenticated_tx: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_connected: Arc::new(Mutex::new(None)),
            on_disconnected: Arc::new(Mutex::new(None)),
            on_peer_list: Arc::new(Mutex::new(None)),
        }
    }

    /// Send a live change to the currently connected peer, if any. The
    /// message is dropped silently if the WS connection is not up or not
    /// authenticated — the sender on the server side is expected to keep
    /// state.
    pub async fn broadcast_live_change(&self, entity: SyncEntity) {
        let guard = self.authenticated_tx.lock().await;
        if let Some(tx) = guard.as_ref() {
            let change_id = generate_id();
            let msg = LanSyncMessage::LiveChange { change_id, entity };
            let _ = tx.send(Message::Text(serialize_message(&msg)));
        }
    }

    pub async fn set_on_change(&self, handler: ClientOnChangeCallback) {
        *self.on_change.lock().await = Some(handler);
    }

    pub async fn set_on_connected(&self, handler: ClientOnConnectedCallback) {
        *self.on_connected.lock().await = Some(handler);
    }

    pub async fn set_on_disconnected(&self, handler: ClientOnDisconnectedCallback) {
        *self.on_disconnected.lock().await = Some(handler);
    }

    pub async fn set_on_peer_list(&self, handler: ClientOnPeerListCallback) {
        *self.on_peer_list.lock().await = Some(handler);
    }

    pub fn peer_device_id(&self) -> String {
        // We can't async here, but peer is set at construction
        // Return from initial peer record
        self.device_id.clone() // placeholder
    }

    /// Start connecting to the peer. Spawns a background task.
    pub fn start(&self) {
        self.stopped.store(false, std::sync::atomic::Ordering::Relaxed);
        let storage = self.storage.clone();
        let peer = self.peer.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let space_id = self.space_id.clone();
        let own_addresses = self.own_addresses.clone();
        let stopped = self.stopped.clone();
        let authenticated_tx = self.authenticated_tx.clone();
        let on_change = self.on_change.clone();
        let on_connected = self.on_connected.clone();
        let on_disconnected = self.on_disconnected.clone();
        let on_peer_list = self.on_peer_list.clone();

        tokio::spawn(async move {
            let mut reconnect_delay = RECONNECT_BASE_MS;

            loop {
                if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                let peer_record = peer.read().await.clone();
                let addresses = peer_record.addresses.clone();

                if addresses.is_empty() {
                    eprintln!("{TAG} No addresses for peer {}, scheduling reconnect", peer_record.device_name);
                    tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
                    reconnect_delay = (reconnect_delay as f64 * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
                    continue;
                }

                // Race connections to all addresses
                let result = race_connect(&addresses).await;

                match result {
                    Some((ws_stream, winning_addr)) => {
                        eprintln!("{TAG} Connected to {} via {winning_addr}", peer_record.device_name);

                        // Update last_address
                        {
                            let mut p = peer.write().await;
                            p.last_address = Some(winning_addr);
                        }

                        reconnect_delay = RECONNECT_BASE_MS;

                        let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();

                        // Send hello
                        let hello = LanSyncMessage::Hello {
                            protocol_version: PROTOCOL_VERSION,
                            device_id: device_id.clone(),
                            device_name: device_name.clone(),
                            space_id: space_id.clone(),
                            addresses: Some(own_addresses.clone()),
                        };
                        let hello_json = serialize_message(&hello);
                        if ws_sink.send(Message::Text(hello_json)).await.is_err() {
                            continue;
                        }

                        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                        // Writer task
                        let writer = tokio::spawn(async move {
                            while let Some(msg) = rx.recv().await {
                                if ws_sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                        });

                        // Read messages
                        let mut authenticated = false;
                        let mut _sync_complete = false;
                        let mut queued_live_changes: Vec<SyncEntity> = Vec::new();
                        let mut peer_device_id_actual = String::new();

                        while let Some(Ok(msg)) = ws_stream_rx.next().await {
                            match msg {
                                Message::Text(text) => {
                                    if let Some(sync_msg) = deserialize_message(&text) {
                                        match sync_msg {
                                            LanSyncMessage::Hello {
                                                device_id: server_device_id,
                                                device_name: server_device_name,
                                                ..
                                            } => {
                                                // Reject self-connect: if the server's hello
                                                // claims our own device_id, we accidentally
                                                // connected to our own SyncServer (stale phantom
                                                // peer record pointing at our own LAN IP). Close
                                                // and stop retrying — this peer record is a
                                                // self-reference and should be evicted.
                                                if !server_device_id.is_empty() && server_device_id == device_id {
                                                    eprintln!("{TAG} Rejecting self-connect to {server_device_name} ({server_device_id})");
                                                    stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                                                    break;
                                                }

                                                authenticated = true;
                                                peer_device_id_actual = server_device_id.clone();
                                                *authenticated_tx.lock().await = Some(tx.clone());
                                                eprintln!("{TAG} Authenticated with {server_device_name} ({server_device_id})");
                                                if let Some(handler) = on_connected.lock().await.as_ref() {
                                                    handler(server_device_id, server_device_name);
                                                }

                                                // Send version vector
                                                let vector = load_version_vector(&storage).await;
                                                send_msg(&tx, &LanSyncMessage::VersionVector { vector });
                                            }

                                            LanSyncMessage::VersionVector { vector: remote_vector } => {
                                                if !authenticated { continue; }

                                                let mut local_vector = load_version_vector(&storage).await;
                                                let all_entities = storage.load_entities(&local_vector).await;

                                                for entity in &all_entities {
                                                    if !local_vector.contains_key(&entity.id) {
                                                        local_vector.insert(entity.id.clone(), entity.hlc.clone());
                                                    }
                                                }
                                                save_version_vector(&storage, &local_vector).await;

                                                let mut to_send: Vec<SyncEntity> = Vec::new();
                                                for entity in &all_entities {
                                                    match remote_vector.get(&entity.id) {
                                                        None => to_send.push(entity.clone()),
                                                        Some(rh) if HLC::is_newer(&entity.hlc, rh) => to_send.push(entity.clone()),
                                                        _ => {}
                                                    }
                                                }

                                                if to_send.is_empty() {
                                                    send_msg(&tx, &LanSyncMessage::SyncChanges {
                                                        batch_id: generate_id(),
                                                        entities: vec![],
                                                        is_last: true,
                                                    });
                                                } else {
                                                    let batches = split_into_batches(&to_send);
                                                    for (i, batch) in batches.iter().enumerate() {
                                                        send_msg(&tx, &LanSyncMessage::SyncChanges {
                                                            batch_id: generate_id(),
                                                            entities: batch.clone(),
                                                            is_last: i == batches.len() - 1,
                                                        });
                                                    }
                                                }

                                                _sync_complete = true;
                                                // Flush queued live changes
                                                for entity in queued_live_changes.drain(..) {
                                                    let change_id = generate_id();
                                                    send_msg(&tx, &LanSyncMessage::LiveChange {
                                                        change_id,
                                                        entity,
                                                    });
                                                }
                                            }

                                            LanSyncMessage::SyncChanges { batch_id, entities, is_last } => {
                                                if !authenticated { continue; }

                                                let mut local_vector = load_version_vector(&storage).await;
                                                let mut accepted = 0;

                                                for entity in &entities {
                                                    let should_apply = match local_vector.get(&entity.id) {
                                                        None => true,
                                                        Some(lh) => HLC::is_newer(&entity.hlc, lh),
                                                    };
                                                    if should_apply {
                                                        storage.apply_entity(entity).await;
                                                        if entity.deleted == Some(true) {
                                                            local_vector.remove(&entity.id);
                                                        } else {
                                                            local_vector.insert(entity.id.clone(), entity.hlc.clone());
                                                        }
                                                        accepted += 1;
                                                        if let Some(handler) = on_change.lock().await.as_ref() {
                                                            handler(entity.clone());
                                                        }
                                                    }
                                                }

                                                save_version_vector(&storage, &local_vector).await;
                                                send_msg(&tx, &LanSyncMessage::SyncAck { batch_id, accepted });

                                                if is_last {
                                                    eprintln!("{TAG} Received all sync batches from server");
                                                }
                                            }

                                            LanSyncMessage::SyncAck { .. } => {}

                                            LanSyncMessage::LiveChange { change_id, entity } => {
                                                if !authenticated { continue; }

                                                let mut local_vector = load_version_vector(&storage).await;
                                                let should_apply = match local_vector.get(&entity.id) {
                                                    None => true,
                                                    Some(lh) => HLC::is_newer(&entity.hlc, lh),
                                                };

                                                if should_apply {
                                                    storage.apply_entity(&entity).await;
                                                    if entity.deleted == Some(true) {
                                                        local_vector.remove(&entity.id);
                                                    } else {
                                                        local_vector.insert(entity.id.clone(), entity.hlc.clone());
                                                    }
                                                    save_version_vector(&storage, &local_vector).await;

                                                    if let Some(handler) = on_change.lock().await.as_ref() {
                                                        handler(entity);
                                                    }
                                                }

                                                send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
                                            }

                                            LanSyncMessage::LiveAck { .. } => {}

                                            LanSyncMessage::PeerList { peers: peer_list } => {
                                                if !authenticated { continue; }
                                                if let Some(handler) = on_peer_list.lock().await.as_ref() {
                                                    handler(peer_list);
                                                }
                                            }

                                            LanSyncMessage::Ping { ts } => {
                                                send_msg(&tx, &LanSyncMessage::Pong { ts });
                                            }

                                            LanSyncMessage::Pong { .. } => {}
                                        }
                                    }
                                }
                                Message::Close(_) => break,
                                _ => {}
                            }
                        }

                        // Connection closed
                        writer.abort();
                        *authenticated_tx.lock().await = None;

                        if authenticated {
                            eprintln!("{TAG} Disconnected from peer");
                            if let Some(handler) = on_disconnected.lock().await.as_ref() {
                                handler(peer_device_id_actual);
                            }
                        }
                    }
                    None => {
                        eprintln!("{TAG} All addresses failed for {}", peer_record.device_name);
                    }
                }

                if stopped.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                // Reconnect with backoff
                let jitter = rand::random::<f64>() * 1000.0;
                let delay = (reconnect_delay as f64 + jitter).min(RECONNECT_MAX_MS as f64);
                tokio::time::sleep(Duration::from_millis(delay as u64)).await;
                reconnect_delay = ((reconnect_delay as f64) * 1.5).min(RECONNECT_MAX_MS as f64) as u64;
            }
        });
    }

    /// Stop the client and don't reconnect.
    pub fn stop(&self) {
        self.stopped.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Replace the current peer record (typically used when the beacon
    /// discovers updated addresses for a known device).
    pub async fn update_peer(&self, peer: PeerRecord) {
        *self.peer.write().await = peer;
    }

    /// Snapshot of the peer this client is dialling.
    pub async fn current_peer(&self) -> PeerRecord {
        self.peer.read().await.clone()
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn send_msg(tx: &mpsc::UnboundedSender<Message>, msg: &LanSyncMessage) {
    let json = serialize_message(msg);
    let _ = tx.send(Message::Text(json));
}

/// Race connections to all addresses; return the first successful WebSocket + winning address.
async fn race_connect(
    addresses: &[String],
) -> Option<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
)> {
    use tokio::sync::oneshot;

    if addresses.is_empty() {
        return None;
    }

    let (result_tx, result_rx) = oneshot::channel();
    let result_tx = Arc::new(Mutex::new(Some(result_tx)));
    let mut handles = Vec::new();

    for addr in addresses {
        let url = format!("ws://{addr}");
        let addr_clone = addr.clone();
        let result_tx = result_tx.clone();

        let handle = tokio::spawn(async move {
            let connect_future = connect_async(&url);
            if let Ok(Ok((ws_stream, _))) =
                tokio::time::timeout(Duration::from_millis(CONNECT_TIMEOUT_MS), connect_future)
                    .await
            {
                let mut guard = result_tx.lock().await;
                if let Some(tx) = guard.take() {
                    let _ = tx.send((ws_stream, addr_clone));
                }
            }
        });
        handles.push(handle);
    }

    match result_rx.await {
        Ok(result) => {
            // Cancel other candidates
            for handle in handles {
                handle.abort();
            }
            Some(result)
        }
        Err(_) => None,
    }
}
