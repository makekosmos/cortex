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
const REMOVED_PEERS_KEY: &str = "sync.removed_peers";
const SYNC_LOAD_PAGE_SIZE: usize = 100;

// ---------------------------------------------------------------------------
// StorageBackend trait
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity>;
    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity>;
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String>;
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
    auth_secret: Arc<RwLock<Option<String>>>,
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
            auth_secret: Arc::new(RwLock::new(None)),
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

    pub async fn get_removed_peer_ids(&self) -> Vec<String> {
        load_removed_peer_ids(&self.storage).await
    }

    pub async fn block_peer(&self, device_id: &str) {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return;
        }
        let mut removed = load_removed_peer_ids(&self.storage).await;
        if !removed.iter().any(|id| id == device_id) {
            removed.push(device_id.to_string());
            save_removed_peer_ids(&self.storage, &removed).await;
        }
        let mut known = self.known_peer_records.lock().await;
        let before = known.len();
        known.retain(|peer| peer.device_id != device_id);
        if known.len() != before {
            save_known_peers(&self.storage, &known).await;
        }
    }

    /// Block a peer and explicitly tear down any live inbound sessions for it.
    /// Returns the number of active sessions that were closed.
    pub async fn disconnect_peer(&self, device_id: &str) -> usize {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return 0;
        }
        self.block_peer(device_id).await;

        let mut peers = self.peers.lock().await;
        let peer_ids: Vec<usize> = peers
            .iter()
            .filter_map(|(peer_id, peer)| {
                if peer.device_id == device_id {
                    Some(*peer_id)
                } else {
                    None
                }
            })
            .collect();

        let mut closed = 0usize;
        for peer_id in &peer_ids {
            if let Some(peer) = peers.get_mut(peer_id) {
                if peer.authenticated {
                    peer.authenticated = false;
                    let _ = peer.tx.send(Message::Close(None));
                    closed += 1;
                }
            }
        }
        for peer_id in peer_ids {
            peers.remove(&peer_id);
        }
        closed
    }

    async fn is_peer_blocked(&self, device_id: &str) -> bool {
        load_removed_peer_ids(&self.storage)
            .await
            .iter()
            .any(|id| id == device_id)
    }

    /// Dedup connected peers by device_id. Matches the `getConnectedPeerEntries`
    /// semantics of the TS sync server. Order: LinkedHashMap insertion order.
    pub async fn get_connected_peer_entries(&self) -> Vec<(String, String)> {
        let peers = self.peers.lock().await;
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for peer in peers.values() {
            if !peer.authenticated {
                continue;
            }
            if peer.device_id.is_empty() {
                continue;
            }
            if !seen.contains_key(&peer.device_id) {
                order.push(peer.device_id.clone());
            }
            seen.insert(peer.device_id.clone(), peer.device_name.clone());
        }
        order
            .into_iter()
            .map(|id| {
                let name = seen.remove(&id).unwrap_or_default();
                (id, name)
            })
            .collect()
    }

    /// True if any authenticated session matches the given `device_id`.
    pub async fn is_connected_to(&self, device_id: &str) -> bool {
        let peers = self.peers.lock().await;
        peers
            .values()
            .any(|p| p.authenticated && p.device_id == device_id)
    }

    /// Record an externally-connected peer (e.g. a `SyncClient` we just dialled
    /// out to) so `get_known_peers` reflects the merged set. Mirrors the
    /// TS `registerExternalPeer` helper.
    pub async fn register_external_peer(
        &self,
        device_id: &str,
        device_name: &str,
        addresses: Vec<String>,
    ) {
        let my_device_id = self.device_id.read().await.clone();
        let my_addresses = self.own_addresses.read().await.clone();

        if device_id == my_device_id {
            return;
        }
        if !addresses.is_empty() && addresses.iter().all(|a| my_addresses.contains(a)) {
            return;
        }

        if self.is_peer_blocked(device_id).await {
            return;
        }
        let new_record = PeerRecord {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            addresses,
            last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            last_address: None,
        };
        let mut known = self.known_peer_records.lock().await;
        *known = merge_peer_records(&known, &[new_record]);
        save_known_peers(&self.storage, &known).await;
    }

    /// Replace the current own-addresses snapshot. Used when the host network
    /// changes between `start_sync` calls or when an embedder wants to refresh
    /// the routable set.
    pub async fn set_own_addresses(&self, addresses: Vec<String>) {
        *self.own_addresses.write().await = addresses;
    }

    pub async fn set_auth_secret(&self, auth_secret: Option<String>) {
        *self.auth_secret.write().await = normalize_auth_secret(auth_secret);
    }

    pub async fn get_device_id(&self) -> String {
        self.device_id.read().await.clone()
    }

    pub async fn get_device_name(&self) -> String {
        self.device_name.read().await.clone()
    }

    /// Start the WS server on the default LAN sync port.
    pub async fn start(
        &self,
        space_id: &str,
        device_id: &str,
        device_name: Option<&str>,
        own_addresses: Option<Vec<String>>,
    ) -> Result<(), String> {
        self.start_with_addr(
            space_id,
            device_id,
            device_name,
            own_addresses,
            &format!("0.0.0.0:{LAN_SYNC_PORT}"),
        )
        .await
    }

    /// Start the WS server on an explicit bind address. Used by integration
    /// tests that need to avoid port conflicts and by embedders that want a
    /// loopback-only listener.
    pub async fn start_with_addr(
        &self,
        space_id: &str,
        device_id: &str,
        device_name: Option<&str>,
        own_addresses: Option<Vec<String>>,
        bind_addr: &str,
    ) -> Result<(), String> {
        *self.space_id.write().await = space_id.to_string();
        *self.device_id.write().await = device_id.to_string();
        if let Some(name) = device_name {
            *self.device_name.write().await = name.to_string();
        }
        if let Some(addrs) = own_addresses {
            *self.own_addresses.write().await = addrs;
        }

        // Load known peers and evict stale self-references.
        self.load_known_peers().await;
        self.purge_self_peers().await;

        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| format!("Failed to bind {bind_addr}: {e}"))?;

        eprintln!("{TAG} Server listening on {bind_addr}");

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let peers = self.peers.clone();
        let storage = self.storage.clone();
        let space_id = self.space_id.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let own_addresses = self.own_addresses.clone();
        let auth_secret = self.auth_secret.clone();
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
                                let auth_secret_reader = auth_secret.clone();
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
                                                        &auth_secret_reader,
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
            origin_device_id: None,
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
                        let removed = load_removed_peer_ids(&self.storage).await;
                        *self.known_peer_records.lock().await = peers
                            .into_iter()
                            .filter(|peer| !removed.iter().any(|id| id == &peer.device_id))
                            .collect();
                    }
                }
            }
        }
    }

    /// Purge stale self-referencing peer records. A record is "self" if:
    ///   - its device_id matches our current device_id, OR
    ///   - every address in its list is one of our own addresses (prior-run
    ///     phantom with a different device_id).
    ///
    /// Called on start after loading known peers. Prevents self-connect loops
    /// where stale records from dev iterations fire SyncClients that connect
    /// to our own SyncServer.
    pub async fn purge_self_peers(&self) -> usize {
        let my_device_id = self.device_id.read().await.clone();
        let my_addresses = self.own_addresses.read().await.clone();
        let mut known = self.known_peer_records.lock().await;
        let before = known.len();
        known.retain(|p| {
            if p.device_id == my_device_id {
                return false;
            }
            if p.addresses.is_empty() {
                return true;
            }
            let all_ours = p.addresses.iter().all(|a| my_addresses.contains(a));
            !all_ours
        });
        let removed = before - known.len();
        if removed > 0 {
            eprintln!("{TAG} Purged {removed} stale self-peer records");
            save_known_peers(&self.storage, &known).await;
        }
        removed
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

async fn load_removed_peer_ids(storage: &Arc<dyn StorageBackend>) -> Vec<String> {
    match storage.get_kv(REMOVED_PEERS_KEY).await {
        Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        None => Vec::new(),
    }
}

async fn save_removed_peer_ids(storage: &Arc<dyn StorageBackend>, peer_ids: &[String]) {
    let json = serde_json::to_string(peer_ids).unwrap_or_default();
    storage.set_kv(REMOVED_PEERS_KEY, &json).await;
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
    auth_secret: &Arc<RwLock<Option<String>>>,
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
            auth_nonce,
            auth_hmac,
        } => {
            if protocol_version != PROTOCOL_VERSION {
                eprintln!(
                    "{TAG} Protocol version mismatch: {protocol_version} vs {PROTOCOL_VERSION}"
                );
                return;
            }

            let my_device_id = device_id.read().await.clone();
            let my_device_name = device_name.read().await.clone();
            let my_space_id = space_id.read().await.clone();
            let my_addresses = own_addresses.read().await.clone();

            if let Some(secret) = auth_secret.read().await.clone() {
                let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
                    (Some(nonce), Some(hmac)) => {
                        verify_hello_auth_hmac(&secret, &my_space_id, &peer_device_id, nonce, hmac)
                    }
                    _ => false,
                };
                if !valid {
                    eprintln!("{TAG} Rejecting peer hello with invalid HMAC");
                    let mut peers_guard = peers.lock().await;
                    if let Some(peer) = peers_guard.remove(&peer_id) {
                        let _ = peer.tx.send(Message::Close(None));
                    }
                    return;
                }
            }

            // Reject self-connect: a client claiming our own device_id is us
            // (stale phantom peer in storage or address looping back to us).
            if !peer_device_id.is_empty() && peer_device_id == my_device_id {
                eprintln!(
                    "{TAG} Rejecting self-connect: client claims our device_id {peer_device_id}"
                );
                let mut peers_guard = peers.lock().await;
                peers_guard.remove(&peer_id);
                return;
            }
            let removed = load_removed_peer_ids(storage).await;
            if !peer_device_id.is_empty() && removed.iter().any(|id| id == &peer_device_id) {
                eprintln!("{TAG} Rejecting blocked peer: {peer_device_id}");
                let mut peers_guard = peers.lock().await;
                peers_guard.remove(&peer_id);
                return;
            }

            // Evict any prior authenticated sessions from the same device — a
            // new hello means a fresh connection, and the old session is stale.
            // Mark stale sessions unauthenticated so their close handler
            // doesn't fire an extra on_peer_disconnected.
            {
                let mut peers_guard = peers.lock().await;
                let stale_ids: Vec<usize> = peers_guard
                    .iter()
                    .filter_map(|(id, p)| {
                        if *id != peer_id && p.authenticated && p.device_id == peer_device_id {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .collect();
                for stale_id in stale_ids {
                    if let Some(stale) = peers_guard.get_mut(&stale_id) {
                        eprintln!(
                            "{TAG} Evicting stale session for {} (superseded by new hello)",
                            stale.device_name
                        );
                        stale.authenticated = false;
                        let _ = stale.tx.send(Message::Close(None));
                    }
                    peers_guard.remove(&stale_id);
                }
            }

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

            // Update known peer record — skip if it's a self-reference
            // (device_id match or all-self address list).
            {
                let peer_addresses = addresses.clone().unwrap_or_default();
                let is_self = peer_device_id == my_device_id
                    || (!peer_addresses.is_empty()
                        && peer_addresses.iter().all(|a| my_addresses.contains(a)));
                if !is_self {
                    let new_record = PeerRecord {
                        device_id: peer_device_id.clone(),
                        device_name: peer_device_name,
                        addresses: peer_addresses,
                        last_seen: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        last_address: None,
                    };
                    let removed = load_removed_peer_ids(storage).await;
                    if !removed.iter().any(|id| id == &new_record.device_id) {
                        let mut known = known_peer_records.lock().await;
                        *known = merge_peer_records(&known, &[new_record]);
                        save_known_peers(storage, &known).await;
                    }
                }
            }

            // Reply hello
            let (reply_auth_nonce, reply_auth_hmac) = match auth_secret.read().await.clone() {
                Some(secret) => {
                    let nonce = generate_auth_nonce();
                    let hmac =
                        compute_hello_auth_hmac(&secret, &my_space_id, &my_device_id, &nonce);
                    (Some(nonce), Some(hmac))
                }
                None => (None, None),
            };
            send_msg(
                &tx,
                &LanSyncMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    device_id: my_device_id,
                    device_name: my_device_name,
                    space_id: my_space_id,
                    addresses: Some(my_addresses),
                    auth_nonce: reply_auth_nonce,
                    auth_hmac: reply_auth_hmac,
                },
            );

            if let Some(handler) = on_peer_connect.lock().await.as_ref() {
                handler(peer_device_id);
            }

            // Send version vector
            let vector = load_version_vector(storage).await;
            send_msg(
                &tx,
                &LanSyncMessage::VersionVector {
                    vector,
                    origin_device_id: None,
                },
            );

            // Send peer list after short delay
            let known = known_peer_records.lock().await.clone();
            let tx_delayed = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                send_msg(&tx_delayed, &LanSyncMessage::PeerList { peers: known });
            });
        }

        LanSyncMessage::VersionVector {
            vector: remote_vector,
            ..
        } => {
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

            let mut vector_updated = false;
            let mut offset = 0;
            loop {
                let mut load_vector = local_vector.clone();
                merge_usage_cursors(&mut load_vector, &remote_vector);
                let entities = storage
                    .load_entities_page(&load_vector, offset, SYNC_LOAD_PAGE_SIZE)
                    .await;
                if entities.is_empty() {
                    break;
                }
                offset += entities.len();
                let mut to_send = Vec::new();
                for entity in entities {
                    if !is_usage_entity(&entity) && !local_vector.contains_key(&entity.id) {
                        local_vector.insert(entity.id.clone(), entity.hlc.clone());
                        vector_updated = true;
                    }
                    if should_send_entity(&remote_vector, &entity) {
                        to_send.push(entity);
                    }
                }
                for batch in split_into_batches(&to_send) {
                    send_msg(
                        &tx,
                        &LanSyncMessage::SyncChanges {
                            batch_id: generate_id(),
                            entities: batch,
                            is_last: false,
                            origin_device_id: None,
                        },
                    );
                }
            }
            if vector_updated {
                save_version_vector(storage, &local_vector).await;
            }
            send_msg(
                &tx,
                &LanSyncMessage::SyncChanges {
                    batch_id: generate_id(),
                    entities: vec![],
                    is_last: true,
                    origin_device_id: None,
                },
            );

            // Mark sync complete
            {
                let mut peers_guard = peers.lock().await;
                if let Some(peer) = peers_guard.get_mut(&peer_id) {
                    peer.sync_complete = true;
                    // Flush queued live changes
                    let queued: Vec<SyncEntity> = peer.queued_live_changes.drain(..).collect();
                    for entity in queued {
                        let change_id = generate_id();
                        send_msg(
                            &peer.tx,
                            &LanSyncMessage::LiveChange {
                                change_id,
                                entity,
                                origin_device_id: None,
                            },
                        );
                    }
                }
            }
        }

        LanSyncMessage::SyncChanges {
            batch_id,
            entities,
            is_last,
            ..
        } => {
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
            let mut vector_updated = false;

            let peer_device_id = {
                let peers_guard = peers.lock().await;
                peers_guard
                    .get(&peer_id)
                    .map(|p| p.device_id.clone())
                    .unwrap_or_default()
            };

            for entity in &entities {
                let should_apply = is_usage_entity(entity)
                    || local_vector
                        .get(&entity.id)
                        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

                if should_apply {
                    match storage.apply_entity(entity).await {
                        Ok(()) => {
                            vector_updated |= observe_non_usage_entity(&mut local_vector, entity);
                            accepted += 1;

                            if let Some(handler) = on_change.lock().await.as_ref() {
                                handler(entity.clone());
                            }

                            // Broadcast to other peers
                            broadcast_to_others(peers, &entity.clone(), Some(&peer_device_id))
                                .await;
                        }
                        Err(e) => {
                            eprintln!(
                                "{TAG} Failed to apply sync entity {}:{}: {e}",
                                entity.entity_type, entity.id
                            );
                        }
                    }
                }
            }

            if vector_updated {
                merge_usage_cursors(&mut local_vector, &load_version_vector(storage).await);
                save_version_vector(storage, &local_vector).await;
            }
            send_msg(&tx, &LanSyncMessage::SyncAck { batch_id, accepted });

            if is_last {
                eprintln!("{TAG} Received all sync batches from peer {peer_id}");
            }
        }

        LanSyncMessage::SyncAck { .. } => {
            // ACK received -- in a full implementation, resolve pending ACK timers
        }

        LanSyncMessage::LiveChange {
            change_id, entity, ..
        } => {
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
            let should_apply = is_usage_entity(&entity)
                || local_vector
                    .get(&entity.id)
                    .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));

            if should_apply {
                match storage.apply_entity(&entity).await {
                    Ok(()) => {
                        if observe_non_usage_entity(&mut local_vector, &entity) {
                            merge_usage_cursors(
                                &mut local_vector,
                                &load_version_vector(storage).await,
                            );
                            save_version_vector(storage, &local_vector).await;
                        }

                        if let Some(handler) = on_change.lock().await.as_ref() {
                            handler(entity.clone());
                        }

                        let peer_device_id = {
                            let peers_guard = peers.lock().await;
                            peers_guard
                                .get(&peer_id)
                                .map(|p| p.device_id.clone())
                                .unwrap_or_default()
                        };
                        broadcast_to_others(peers, &entity, Some(&peer_device_id)).await;
                    }
                    Err(e) => {
                        eprintln!(
                            "{TAG} Failed to apply live sync entity {}:{}: {e}",
                            entity.entity_type, entity.id
                        );
                    }
                }
            }

            send_msg(&tx, &LanSyncMessage::LiveAck { change_id });
        }

        LanSyncMessage::LiveAck { .. } => {}

        LanSyncMessage::PeerList {
            peers: incoming_peers,
        } => {
            let authenticated = {
                let peers_guard = peers.lock().await;
                peers_guard
                    .get(&peer_id)
                    .map(|p| p.authenticated)
                    .unwrap_or(false)
            };
            if !authenticated {
                return;
            }

            let my_device_id = device_id.read().await.clone();
            let my_addresses = own_addresses.read().await.clone();
            // Filter self-references: our own device_id, or any record whose
            // addresses are all ours (stale phantom from prior runs).
            let filtered: Vec<PeerRecord> = incoming_peers
                .into_iter()
                .filter(|p| {
                    if p.device_id == my_device_id {
                        return false;
                    }
                    if !p.addresses.is_empty()
                        && p.addresses.iter().all(|a| my_addresses.contains(a))
                    {
                        return false;
                    }
                    true
                })
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
                eprintln!(
                    "{TAG} Peer list updated: {before_count} -> {} known peers",
                    known.len()
                );
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
        origin_device_id: None,
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    // Minimal in-memory backend for unit tests.
    struct MemBackend {
        kv: StdMutex<HashMap<String, String>>,
        entities: StdMutex<HashMap<String, SyncEntity>>,
    }

    impl MemBackend {
        fn new() -> Self {
            Self {
                kv: StdMutex::new(HashMap::new()),
                entities: StdMutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl StorageBackend for MemBackend {
        async fn load_entities(&self, _vector: &VersionVector) -> Vec<SyncEntity> {
            self.entities.lock().unwrap().values().cloned().collect()
        }
        async fn load_entities_page(
            &self,
            _vector: &VersionVector,
            offset: usize,
            limit: usize,
        ) -> Vec<SyncEntity> {
            self.entities
                .lock()
                .unwrap()
                .values()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect()
        }
        async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
            if entity.deleted == Some(true) {
                self.entities.lock().unwrap().remove(&entity.id);
            } else {
                self.entities
                    .lock()
                    .unwrap()
                    .insert(entity.id.clone(), entity.clone());
            }
            Ok(())
        }
        async fn get_kv(&self, key: &str) -> Option<String> {
            self.kv.lock().unwrap().get(key).cloned()
        }
        async fn set_kv(&self, key: &str, value: &str) {
            self.kv
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
    }

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
    async fn purge_self_peers_drops_our_device_id() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["192.168.1.10:21531".to_string()];

        *server.known_peer_records.lock().await = vec![
            peer_rec("me", &["192.168.1.10:21531"]),
            peer_rec("other", &["192.168.1.20:21531"]),
        ];

        let removed = server.purge_self_peers().await;
        assert_eq!(removed, 1);
        let known = server.get_known_peers().await;
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].device_id, "other");
    }

    #[tokio::test]
    async fn purge_self_peers_drops_records_with_all_our_addresses() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec![
            "192.168.1.10:21531".to_string(),
            "10.0.0.5:21531".to_string(),
        ];

        *server.known_peer_records.lock().await = vec![
            // Different device_id, but every address is ours -> phantom self.
            peer_rec("phantom", &["192.168.1.10:21531", "10.0.0.5:21531"]),
            // Keep: one of the addresses is not ours.
            peer_rec("real", &["192.168.1.10:21531", "192.168.1.99:21531"]),
        ];

        let removed = server.purge_self_peers().await;
        assert_eq!(removed, 1);
        let ids: Vec<String> = server
            .get_known_peers()
            .await
            .into_iter()
            .map(|p| p.device_id)
            .collect();
        assert_eq!(ids, vec!["real".to_string()]);
    }

    #[tokio::test]
    async fn register_external_peer_rejects_self() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["10.0.0.1:21531".to_string()];

        server
            .register_external_peer("me", "Me", vec!["10.0.0.1:21531".to_string()])
            .await;
        // Also: all-self addresses under a different device_id.
        server
            .register_external_peer("phantom", "Phantom", vec!["10.0.0.1:21531".to_string()])
            .await;

        assert!(
            server.get_known_peers().await.is_empty(),
            "register_external_peer must drop self and all-self-address records",
        );
    }

    #[tokio::test]
    async fn register_external_peer_stores_routable_peer() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["10.0.0.1:21531".to_string()];

        server
            .register_external_peer("other", "Other", vec!["192.168.1.20:21531".to_string()])
            .await;

        let known = server.get_known_peers().await;
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].device_id, "other");
        assert_eq!(known[0].addresses, vec!["192.168.1.20:21531".to_string()]);
    }

    #[tokio::test]
    async fn block_peer_persists_removed_peer_and_filters_known_list() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.known_peer_records.lock().await = vec![peer_rec("other", &["192.168.1.20:21531"])];

        server.block_peer("other").await;

        assert!(server.get_known_peers().await.is_empty());
        assert_eq!(
            server.get_removed_peer_ids().await,
            vec!["other".to_string()]
        );
        assert!(server.is_peer_blocked("other").await);
    }

    #[tokio::test]
    async fn register_external_peer_ignores_blocked_peer() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["10.0.0.1:21531".to_string()];
        server.block_peer("blocked").await;

        server
            .register_external_peer("blocked", "Blocked", vec!["192.168.1.21:21531".to_string()])
            .await;

        assert!(server.get_known_peers().await.is_empty());
    }

    #[tokio::test]
    async fn disconnect_peer_closes_matching_sessions_and_persists_blocklist() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());

        let (tx1, _rx1) = mpsc::unbounded_channel::<Message>();
        let (tx2, _rx2) = mpsc::unbounded_channel::<Message>();
        let (tx3, _rx3) = mpsc::unbounded_channel::<Message>();

        server.peers.lock().await.insert(
            1,
            PeerState {
                device_id: "blocked".to_string(),
                device_name: "Blocked One".to_string(),
                addresses: vec!["192.168.1.20:21531".to_string()],
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx: tx1,
            },
        );
        server.peers.lock().await.insert(
            2,
            PeerState {
                device_id: "blocked".to_string(),
                device_name: "Blocked Two".to_string(),
                addresses: vec!["192.168.1.21:21531".to_string()],
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx: tx2,
            },
        );
        server.peers.lock().await.insert(
            3,
            PeerState {
                device_id: "keep".to_string(),
                device_name: "Keep".to_string(),
                addresses: vec!["192.168.1.22:21531".to_string()],
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx: tx3,
            },
        );

        let closed = server.disconnect_peer("blocked").await;
        assert_eq!(closed, 2);
        assert_eq!(
            server.get_removed_peer_ids().await,
            vec!["blocked".to_string()]
        );
        assert_eq!(server.connected_peer_count().await, 1);
        assert_eq!(
            server.get_connected_peer_entries().await,
            vec![("keep".to_string(), "Keep".to_string())]
        );
        assert_eq!(server.peers.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn get_connected_peer_entries_dedup_by_device_id() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        // Simulate two authenticated sessions from the same device_id (a race
        // between old + new sessions before eviction settles).
        let (tx, _rx) = mpsc::unbounded_channel::<Message>();
        let state = |device_id: &str| PeerState {
            device_id: device_id.to_string(),
            device_name: "DupDevice".to_string(),
            addresses: vec![],
            authenticated: true,
            sync_complete: true,
            queued_live_changes: vec![],
            tx: tx.clone(),
        };
        server.peers.lock().await.insert(1, state("dup"));
        server.peers.lock().await.insert(2, state("dup"));

        let entries = server.get_connected_peer_entries().await;
        assert_eq!(
            entries.len(),
            1,
            "duplicate device_id sessions should collapse to one entry",
        );
        assert_eq!(entries[0].0, "dup");
    }
}
