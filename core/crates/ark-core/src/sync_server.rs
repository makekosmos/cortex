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
use crate::integration_replication::SignedSyncEnvelope;
use crate::protocol::*;
use crate::types::*;

#[path = "sync_server_core.rs"]
mod sync_server_core;
#[path = "sync_server_inbound.rs"]
mod sync_server_inbound;
#[path = "sync_server_messages.rs"]
mod sync_server_messages;
#[path = "sync_server_peers.rs"]
mod sync_server_peers;
#[cfg(test)]
#[path = "sync_server_reject_tests.rs"]
mod sync_server_reject_tests;
#[cfg(test)]
#[path = "sync_server_route_tests.rs"]
mod sync_server_route_tests;
#[cfg(test)]
#[path = "sync_server_tests.rs"]
mod sync_server_tests;

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
    async fn authorized_transport_public_key(&self, _device_id: &str) -> Option<String> {
        None
    }
    async fn validate_outbound_signed_integration_frame(
        &self,
        _frame: &SignedSyncEnvelope,
        _expected_space_id: &str,
        _expected_origin_node_id: &str,
    ) -> Result<(), String> {
        Err("signed integration outbound validation is unsupported by this storage backend".into())
    }
    async fn validate_outbound_signed_integration_frame_with_transport(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        expected_origin_node_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        let _ = (
            frame,
            expected_space_id,
            expected_origin_node_id,
            transport_public_key,
        );
        Err(
            "signed integration outbound transport authorization is unsupported by this storage backend"
                .into(),
        )
    }
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity>;
    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity>;
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String>;
    async fn apply_signed_integration_frame(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        authenticated_peer_id: &str,
        expected_recipient_node_id: &str,
    ) -> Result<(), String> {
        let _ = (
            frame,
            expected_space_id,
            authenticated_peer_id,
            expected_recipient_node_id,
        );
        Err("signed integration replication is unsupported by this storage backend".into())
    }
    async fn apply_signed_integration_frame_with_transport(
        &self,
        frame: &SignedSyncEnvelope,
        expected_space_id: &str,
        authenticated_peer_id: &str,
        expected_recipient_node_id: &str,
        authenticated_transport_public_key: Option<&str>,
    ) -> Result<(), String> {
        let _ = authenticated_transport_public_key;
        self.apply_signed_integration_frame(
            frame,
            expected_space_id,
            authenticated_peer_id,
            expected_recipient_node_id,
        )
        .await
    }
    async fn get_kv(&self, key: &str) -> Option<String>;
    async fn set_kv(&self, key: &str, value: &str);

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        Some(entity.clone())
    }
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
    /// Accept/ticker task, joined by `stop()` so its `storage` Arc (the
    /// shared db connection) is released deterministically on shutdown
    /// rather than whenever the task happens to exit (KOS-270).
    accept_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
    on_new_peer_discovered: Arc<Mutex<Option<OnNewPeerDiscoveredCallback>>>,
    next_peer_id: Arc<std::sync::atomic::AtomicUsize>,
}

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

/// Shared mutable state handed to `handle_message`. Every field has a
/// different owner; the struct only bundles the Arcs the peer reader task
/// already clones per connection.
#[derive(Clone)]
struct MessageContext {
    peers: Arc<Mutex<HashMap<usize, PeerState>>>,
    storage: Arc<dyn StorageBackend>,
    space_id: Arc<RwLock<String>>,
    device_id: Arc<RwLock<String>>,
    device_name: Arc<RwLock<String>>,
    own_addresses: Arc<RwLock<Vec<String>>>,
    auth_secret: Arc<RwLock<Option<String>>>,
    known_peer_records: Arc<Mutex<Vec<PeerRecord>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_new_peer_discovered: Arc<Mutex<Option<OnNewPeerDiscoveredCallback>>>,
}

async fn save_removed_peer_ids(storage: &Arc<dyn StorageBackend>, peer_ids: &[String]) {
    let json = serde_json::to_string(peer_ids).unwrap_or_default();
    storage.set_kv(REMOVED_PEERS_KEY, &json).await;
}

fn send_msg(tx: &mpsc::UnboundedSender<Message>, msg: &LanSyncMessage) {
    let json = serialize_message(msg);
    let _ = tx.send(Message::Text(json));
}

/// Drops a rejected hello's session and closes its socket. A bare return would
/// leave a phantom unauthenticated peer and a half-open connection the dialer
/// waits on forever.
async fn reject_hello(peers: &Mutex<HashMap<usize, PeerState>>, peer_id: usize) {
    if let Some(peer) = peers.lock().await.remove(&peer_id) {
        let _ = peer.tx.send(Message::Close(None));
    }
}

// ---------------------------------------------------------------------------
// Message handler
// ---------------------------------------------------------------------------

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
