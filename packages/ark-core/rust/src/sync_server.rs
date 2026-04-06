use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use crate::hlc::HLC;
use crate::protocol::*;
use crate::types::*;

const TAG: &str = "[SyncServer]";
const VERSION_VECTOR_KEY: &str = "lan_sync.version_vector";
const KNOWN_PEERS_KEY: &str = "sync.peers";

// ---------------------------------------------------------------------------
// StorageBackend trait
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity>;
    async fn apply_entity(&self, entity: &SyncEntity);
    async fn get_kv(&self, key: &str) -> Option<String>;
    async fn set_kv(&self, key: &str, value: &str);
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

pub type OnChangeCallback = Arc<dyn Fn(SyncEntity) + Send + Sync>;
pub type OnPeerConnectCallback = Arc<dyn Fn(String) + Send + Sync>;
pub type OnPeerDisconnectCallback = Arc<dyn Fn(String, usize) + Send + Sync>;
pub type OnNewPeerDiscoveredCallback = Arc<dyn Fn(PeerRecord) + Send + Sync>;

// ---------------------------------------------------------------------------
// Peer state
// ---------------------------------------------------------------------------

struct PeerState {
    device_id: String,
    device_name: String,
    addresses: Vec<String>,
    authenticated: bool,
    sync_complete: bool,
    queued_live_changes: Vec<SyncEntity>,
    tx: mpsc::UnboundedSender<Message>,
}

// ---------------------------------------------------------------------------
// SyncServer
// ---------------------------------------------------------------------------

pub struct SyncServer {
    storage: Arc<dyn StorageBackend>,
    space_id: Arc<RwLock<String>>,
    device_id: Arc<RwLock<String>>,
    device_name: Arc<RwLock<String>>,
    own_addresses: Arc<RwLock<Vec<String>>>,
    peers: Arc<Mutex<HashMap<usize, PeerState>>>,
    known_peer_records: Arc<Mutex<Vec<PeerRecord>>>,
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
    on_new_peer_discovered: Arc<Mutex<Option<OnNewPeerDiscoveredCallback>>>,
    next_peer_id: Arc<std::sync::atomic::AtomicUsize>,
}

impl SyncServer {
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            space_id: Arc::new(RwLock::new(String::new())),
            device_id: Arc::new(RwLock::new(String::new())),
            device_name: Arc::new(RwLock::new("ArkSync".to_string())),
            own_addresses: Arc::new(RwLock::new(Vec::new())),
            peers: Arc::new(Mutex::new(HashMap::new())),
            known_peer_records: Arc::new(Mutex::new(Vec::new())),
            shutdown_tx: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_peer_connect: Arc::new(Mutex::new(None)),
            on_peer_disconnect: Arc::new(Mutex::new(None)),
            on_new_peer_discovered: Arc::new(Mutex::new(None)),
            next_peer_id: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
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

    pub async fn set_on_new_peer_discovered(&self, handler: OnNewPeerDiscoveredCallback) {
        *self.on_new_peer_discovered.lock().await = Some(handler);
    }

    pub async fn connected_peer_count(&self) -> usize {
        let peers = self.peers.lock().await;
        peers.values().filter(|p| p.authenticated).count()
    }

    pub async fn get_known_peers(&self) -> Vec<PeerRecord> {
        self.known_peer_records.lock().await.clone()
    }

    /// Start the WS server.
    pub async fn start(
        &self,
        space_id: &str,
        device_id: &str,
        device_name: Option<&str>,
        own_addresses: Option<Vec<String>>,
    ) -> Result<(), String> {
        *self.space_id.write().await = space_id.to_string();
        *self.device_id.write().await = device_id.to_string();
        if let Some(name) = device_name {
            *self.device_name.write().await = name.to_string();
        }
        if let Some(addrs) = own_addresses {
            *self.own_addresses.write().await = addrs;
        }

        // Load known peers
        self.load_known_peers().await;

        let addr = format!("0.0.0.0:{LAN_SYNC_PORT}");
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind: {e}"))?;

        eprintln!("{TAG} Server listening on port {LAN_SYNC_PORT}");

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let peers = self.peers.clone();
        let storage = self.storage.clone();
        let space_id = self.space_id.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let own_addresses = self.own_addresses.clone();
        let known_peer_records = self.known_peer_records.clone();
        let on_change = self.on_change.clone();
        let on_peer_connect = self.on_peer_connect.clone();
        let on_peer_disconnect = self.on_peer_disconnect.clone();
        let on_new_peer_discovered = self.on_new_peer_discovered.clone();
        let next_peer_id = self.next_peer_id.clone();

        // Ping task
        let peers_ping = peers.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(PING_INTERVAL_MS));
            loop {
                ticker.tick().await;
                let peers_guard = peers_ping.lock().await;
                for peer in peers_guard.values() {
                    if peer.authenticated {
                        let _ = peer.tx.send(Message::Ping(vec![]));
                    }
                }
            }
        });

        // Accept loop
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let peer_id = next_peer_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let ws_stream = match accept_async(stream).await {
                                    Ok(ws) => ws,
                                    Err(e) => {
                                        eprintln!("{TAG} WS accept error: {e}");
                                        continue;
                                    }
                                };

                                let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();
                                let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                                // Writer task
                                tokio::spawn(async move {
                                    while let Some(msg) = rx.recv().await {
                                        if ws_sink.send(msg).await.is_err() {
                                            break;
                                        }
                                    }
                                });

                                let peer_state = PeerState {
                                    device_id: String::new(),
                                    device_name: String::new(),
                                    addresses: Vec::new(),
                                    authenticated: false,
                                    sync_complete: false,
                                    queued_live_changes: Vec::new(),
                                    tx: tx.clone(),
                                };
                                peers.lock().await.insert(peer_id, peer_state);

                                // Reader task
                                let peers_reader = peers.clone();
                                let storage_reader = storage.clone();
                                let space_id_reader = space_id.clone();
                                let device_id_reader = device_id.clone();
                                let device_name_reader = device_name.clone();
                                let own_addresses_reader = own_addresses.clone();
                                let known_peer_records_reader = known_peer_records.clone();
                                let on_change_reader = on_change.clone();
                                let on_peer_connect_reader = on_peer_connect.clone();
                                let on_peer_disconnect_reader = on_peer_disconnect.clone();
                                let on_new_peer_discovered_reader = on_new_peer_discovered.clone();

                                tokio::spawn(async move {
                                    while let Some(Ok(msg)) = ws_stream_rx.next().await {
                                        match msg {
                                            Message::Text(text) => {
                                                if let Some(sync_msg) = deserialize_message(&text) {
                                                    handle_message(
                                                        peer_id,
                                                        sync_msg,
                                                        &peers_reader,
                                                        &storage_reader,
                                                        &space_id_reader,
                                                        &device_id_reader,
                                                        &device_name_reader,
                                                        &own_addresses_reader,
                                                        &known_peer_records_reader,
                                                        &on_change_reader,
                                                        &on_peer_connect_reader,
                                                        &on_new_peer_discovered_reader,
                                                    ).await;
                                                }
                                            }
                                            Message::Close(_) => break,
                                            _ => {}
                                        }
                                    }

                                    // Peer disconnected
                                    let mut peers_guard = peers_reader.lock().await;
                                    if let Some(peer) = peers_guard.remove(&peer_id) {
                                        if peer.authenticated {
                                            eprintln!("{TAG} Peer disconnected: {} ({})", peer.device_name, peer.device_id);
                                            let remaining = peers_guard.values().filter(|p| p.authenticated).count();
                                            drop(peers_guard);
                                            if let Some(handler) = on_peer_disconnect_reader.lock().await.as_ref() {
                                                handler(peer.device_id, remaining);
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("{TAG} Accept error: {e}");
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the server.
    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        let mut peers = self.peers.lock().await;
        peers.clear();
        eprintln!("{TAG} Server stopped");
    }

    /// Broadcast a live change to all authenticated peers.
    pub async fn broadcast_live_change(&self, entity: SyncEntity, exclude_device_id: Option<&str>) {
        let change_id = generate_id();
        let msg = LanSyncMessage::LiveChange {
            change_id,
            entity: entity.clone(),
        };
        let json = serialize_message(&msg);

        let mut peers = self.peers.lock().await;
        for peer in peers.values_mut() {
            if !peer.authenticated {
                continue;
            }
            if let Some(exclude) = exclude_device_id {
                if peer.device_id == exclude {
                    continue;
                }
            }
            if !peer.sync_complete {
                peer.queued_live_changes.push(entity.clone());
                continue;
            }
            let _ = peer.tx.send(Message::Text(json.clone()));
        }
    }

    /// Update the version vector for a locally-mutated entity.
    pub async fn update_entity_hlc(&self, entity_id: &str) -> String {
        let device_id = self.device_id.read().await.clone();
        let hlc_str = HLC::now(&device_id).to_string();
        let mut vector = load_version_vector(&self.storage).await;
        vector.insert(entity_id.to_string(), hlc_str.clone());
        save_version_vector(&self.storage, &vector).await;
        hlc_str
    }

    async fn load_known_peers(&self) {
        if let Some(raw) = self.storage.get_kv(KNOWN_PEERS_KEY).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
                if parsed.is_array() {
                    if let Ok(peers) = serde_json::from_str::<Vec<PeerRecord>>(&raw) {
                        *self.known_peer_records.lock().await = peers;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
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

async fn save_known_peers(storage: &Arc<dyn StorageBackend>, peers: &[PeerRecord]) {
    let json = serde_json::to_string(peers).unwrap_or_default();
    storage.set_kv(KNOWN_PEERS_KEY, &json).await;
}

fn send_msg(tx: &mpsc::UnboundedSender<Message>, msg: &LanSyncMessage) {
    let json = serialize_message(msg);
    let _ = tx.send(Message::Text(json));
}

// ---------------------------------------------------------------------------
// Message handler
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn handle_message(
    peer_id: usize,
    msg: LanSyncMessage,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    storage: &Arc<dyn StorageBackend>,
    space_id: &Arc<RwLock<String>>,
    device_id: &Arc<RwLock<String>>,
    device_name: &Arc<RwLock<String>>,
    own_addresses: &Arc<RwLock<Vec<String>>>,
    known_peer_records: &Arc<Mutex<Vec<PeerRecord>>>,
    on_change: &Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: &Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_new_peer_discovered: &Arc<Mutex<Option<OnNewPeerDiscoveredCallback>>>,
) {
    match msg {
        LanSyncMessage::Hello {
            protocol_version,
            device_id: peer_device_id,
            device_name: peer_device_name,
            space_id: _peer_space_id,
            addresses,
        } => {
            if protocol_version != PROTOCOL_VERSION {
                eprintln!("{TAG} Protocol version mismatch: {protocol_version} vs {PROTOCOL_VERSION}");
                return;
            }

            let my_device_id = device_id.read().await.clone();
            let my_device_name = device_name.read().await.clone();
            let my_space_id = space_id.read().await.clone();
            let my_addresses = own_addresses.read().await.clone();

            let tx = {
                let mut peers_guard = peers.lock().await;
                if let Some(peer) = peers_guard.get_mut(&peer_id) {
                    peer.device_id = peer_device_id.clone();
                    peer.device_name = peer_device_name.clone();
                    peer.addresses = addresses.clone().unwrap_or_default();
                    peer.authenticated = true;
                    peer.tx.clone()
                } else {
                    return;
                }
            };

            eprintln!("{TAG} Peer authenticated: {peer_device_name}");

            // Update known peer record
            {
                let new_record = PeerRecord {
                    device_id: peer_device_id.clone(),
                    device_name: peer_device_name,
                    addresses: addresses.unwrap_or_default(),
                    last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    last_address: None,
                };
                let mut known = known_peer_records.lock().await;
                *known = merge_peer_records(&known, &[new_record]);
                save_known_peers(storage, &known).await;
            }

            // Reply hello
            send_msg(&tx, &LanSyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: my_device_id,
                device_name: my_device_name,
                space_id: my_space_id,
                addresses: Some(my_addresses),
            });

            if let Some(handler) = on_peer_connect.lock().await.as_ref() {
                handler(peer_device_id);
            }

            // Send version vector
            let vector = load_version_vector(storage).await;
            send_msg(&tx, &LanSyncMessage::VersionVector { vector });

            // Send peer list after short delay
            let known = known_peer_records.lock().await.clone();
            let tx_delayed = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                send_msg(&tx_delayed, &LanSyncMessage::PeerList { peers: known });
            });
        }

        LanSyncMessage::VersionVector { vector: remote_vector } => {
            let (tx, authenticated) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.authenticated => (p.tx.clone(), true),
                    _ => return,
                }
            };
            if !authenticated {
                return;
            }

            let mut local_vector = load_version_vector(storage).await;

            // Load all entities and build vector if empty
            let all_entities = storage.load_entities(&local_vector).await;
            let mut vector_updated = false;
            for entity in &all_entities {
                if !local_vector.contains_key(&entity.id) {
                    local_vector.insert(entity.id.clone(), entity.hlc.clone());
                    vector_updated = true;
                }
            }
            if vector_updated {
                save_version_vector(storage, &local_vector).await;
            }

            // Compute what remote needs
            let mut to_send: Vec<SyncEntity> = Vec::new();
            for entity in &all_entities {
                let remote_hlc = remote_vector.get(&entity.id);
                match remote_hlc {
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

            // Mark sync complete
            {
                let mut peers_guard = peers.lock().await;
                if let Some(peer) = peers_guard.get_mut(&peer_id) {
                    peer.sync_complete = true;
                    // Flush queued live changes
                    let queued: Vec<SyncEntity> = peer.queued_live_changes.drain(..).collect();
                    for entity in queued {
                        let change_id = generate_id();
                        send_msg(&peer.tx, &LanSyncMessage::LiveChange {
                            change_id,
                            entity,
                        });
                    }
                }
            }
        }

        LanSyncMessage::SyncChanges { batch_id, entities, is_last } => {
            let (tx, authenticated) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.authenticated => (p.tx.clone(), true),
                    _ => return,
                }
            };
            if !authenticated {
                return;
            }

            let mut local_vector = load_version_vector(storage).await;
            let mut accepted = 0;

            let peer_device_id = {
                let peers_guard = peers.lock().await;
                peers_guard.get(&peer_id).map(|p| p.device_id.clone()).unwrap_or_default()
            };

            for entity in &entities {
                let local_hlc = local_vector.get(&entity.id);
                let should_apply = match local_hlc {
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

                    // Broadcast to other peers
                    broadcast_to_others(peers, &entity.clone(), Some(&peer_device_id)).await;
                }
            }

            save_version_vector(storage, &local_vector).await;
            send_msg(&tx, &LanSyncMessage::SyncAck { batch_id, accepted });

            if is_last {
                eprintln!("{TAG} Received all sync batches from peer {peer_id}");
            }
        }

        LanSyncMessage::SyncAck { .. } => {
            // ACK received -- in a full implementation, resolve pending ACK timers
        }

        LanSyncMessage::LiveChange { change_id, entity } => {
            let (tx, authenticated) = {
                let peers_guard = peers.lock().await;
                match peers_guard.get(&peer_id) {
                    Some(p) if p.authenticated => (p.tx.clone(), true),
                    _ => return,
                }
            };
            if !authenticated {
                return;
            }

            let mut local_vector = load_version_vector(storage).await;
            let local_hlc = local_vector.get(&entity.id);
            let should_apply = match local_hlc {
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
                save_version_vector(storage, &local_vector).await;

                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity.clone());
                }

                let peer_device_id = {
                    let peers_guard = peers.lock().await;
                    peers_guard.get(&peer_id).map(|p| p.device_id.clone()).unwrap_or_default()
                };
                broadcast_to_others(peers, &entity, Some(&peer_device_id)).await;
            }

            send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
        }

        LanSyncMessage::LiveAck { .. } => {}

        LanSyncMessage::PeerList { peers: incoming_peers } => {
            let authenticated = {
                let peers_guard = peers.lock().await;
                peers_guard.get(&peer_id).map(|p| p.authenticated).unwrap_or(false)
            };
            if !authenticated {
                return;
            }

            let my_device_id = device_id.read().await.clone();
            let filtered: Vec<PeerRecord> = incoming_peers
                .into_iter()
                .filter(|p| p.device_id != my_device_id)
                .collect();

            let mut known = known_peer_records.lock().await;
            let before_count = known.len();
            *known = merge_peer_records(&known, &filtered);
            save_known_peers(storage, &known).await;

            // Notify about new peers
            if let Some(handler) = on_new_peer_discovered.lock().await.as_ref() {
                for peer_rec in &*known {
                    if peer_rec.device_id != my_device_id {
                        handler(peer_rec.clone());
                    }
                }
            }

            if known.len() > before_count {
                eprintln!("{TAG} Peer list updated: {before_count} -> {} known peers", known.len());
            }
        }

        LanSyncMessage::Ping { ts } => {
            let tx = {
                let peers_guard = peers.lock().await;
                peers_guard.get(&peer_id).map(|p| p.tx.clone())
            };
            if let Some(tx) = tx {
                send_msg(&tx, &LanSyncMessage::Pong { ts });
            }
        }

        LanSyncMessage::Pong { .. } => {}
    }
}

async fn broadcast_to_others(
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    entity: &SyncEntity,
    exclude_device_id: Option<&str>,
) {
    let change_id = generate_id();
    let msg = LanSyncMessage::LiveChange {
        change_id,
        entity: entity.clone(),
    };
    let json = serialize_message(&msg);

    let peers_guard = peers.lock().await;
    for peer in peers_guard.values() {
        if !peer.authenticated || !peer.sync_complete {
            continue;
        }
        if let Some(exclude) = exclude_device_id {
            if peer.device_id == exclude {
                continue;
            }
        }
        let _ = peer.tx.send(Message::Text(json.clone()));
    }
}
