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

pub use crate::application::sync_storage::SyncReadClass;
pub use crate::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend,
    SyncRuntimeStorage,
};

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
pub(crate) mod sync_server_tests;

const TAG: &str = "[SyncServer]";
const SYNC_LOAD_PAGE_SIZE: usize = 100;

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
    /// OS / product version the peer advertised in its Hello; `None` for
    /// peers built before those fields existed.
    platform: Option<String>,
    app_version: Option<String>,
    authenticated: bool,
    sync_complete: bool,
    in_flight_feed_position: Option<i64>,
    pending_initial_snapshot_ack: Option<String>,
    tx: mpsc::UnboundedSender<Message>,
}

/// One authenticated connection's view of a peer, for `get_connected_peers`
/// and the sync snapshot. Unlike the persisted `PeerRecord` this carries the
/// metadata even when the peer never made it into the known-peer list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerEntry {
    pub device_id: String,
    pub device_name: String,
    pub platform: Option<String>,
    pub app_version: Option<String>,
}

// ---------------------------------------------------------------------------
// SyncServer
// ---------------------------------------------------------------------------

pub struct SyncServer {
    storage: Arc<dyn SyncRuntimeStorage>,
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
    /// Per-connection reader and writer tasks. They hold `storage` and end
    /// only when their socket closes, so `stop()` aborts and joins them.
    conn_tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
    on_new_peer_discovered: Arc<Mutex<Option<OnNewPeerDiscoveredCallback>>>,
    next_peer_id: Arc<std::sync::atomic::AtomicUsize>,
}

async fn load_version_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<VersionVector, String> {
    storage.load_version_vector().await
}

/// Save a version vector built from a possibly stale snapshot: union it
/// with the stored vector first so keys written concurrently (local
/// mutations, integration-replication bumps) are not clobbered.
async fn save_version_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
    vector: &VersionVector,
) -> Result<(), String> {
    let mut merged = load_version_vector(storage).await?;
    merge_vector_entries(&mut merged, vector);
    storage.store_version_vector(&merged).await
}

/// Persist the pull-side version vector after a `SyncChanges` batch: merge
/// the contiguous `@usage:` cursors that `apply_entity` wrote to storage,
/// then — only on the sender's final page — raise them to its
/// complete-through claims. Mid-pull claims would leapfrog refs still in
/// flight, and a lost connection would leave the cursor past entities we
/// never received; merging stored cursors first keeps the claims a floor,
/// never a regression.
pub(crate) async fn persist_pull_vector(
    storage: &Arc<dyn SyncRuntimeStorage>,
    vector: &mut VersionVector,
    vector_updated: bool,
    is_last: bool,
    usage_complete_through: Option<&HashMap<String, u64>>,
) -> Result<(), String> {
    if !(vector_updated || (is_last && usage_complete_through.is_some())) {
        return Ok(());
    }
    merge_usage_cursors(vector, &load_version_vector(storage).await?);
    if is_last {
        if let Some(through) = usage_complete_through {
            apply_usage_complete_through(vector, through);
        }
    }
    save_version_vector(storage, vector).await
}

pub(crate) async fn save_known_peers(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &[PeerRecord],
) -> Result<(), String> {
    storage.store_known_peers(peers).await
}

pub(crate) async fn load_known_peer_records(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<Vec<PeerRecord>, String> {
    Ok(storage.load_known_peers().await?.unwrap_or_default())
}

pub(crate) async fn load_removed_peer_ids(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<Vec<String>, String> {
    storage.load_removed_peer_ids().await
}

/// Re-authorize a device the user previously removed — invoked only when
/// they re-pair by explicitly entering that endpoint's pairing code.
pub(crate) async fn unblock_peer_id(
    storage: &Arc<dyn SyncRuntimeStorage>,
    device_id: &str,
) -> Result<(), String> {
    let mut removed = load_removed_peer_ids(storage).await?;
    if removed.iter().any(|id| id == device_id) {
        removed.retain(|id| id != device_id);
        save_removed_peer_ids(storage, &removed).await?;
    }
    Ok(())
}

/// Drop a peer record outright — for a declined pairing attempt, where the
/// device was never consented to and must not linger as an offline row
/// (distinct from `block_peer`/`removed_peers`, which is a user blacklist).
pub(crate) async fn remove_peer_record(
    storage: &Arc<dyn SyncRuntimeStorage>,
    device_id: &str,
) -> Result<(), String> {
    let mut peers = load_known_peer_records(storage).await?;
    peers.retain(|peer| peer.device_id != device_id);
    save_known_peers(storage, &peers).await
}

pub(crate) async fn load_peer_transport_keys(
    storage: &Arc<dyn SyncRuntimeStorage>,
) -> Result<HashMap<String, String>, String> {
    storage.load_peer_transport_keys().await
}

pub(crate) async fn save_peer_transport_keys(
    storage: &Arc<dyn SyncRuntimeStorage>,
    keys: &HashMap<String, String>,
) -> Result<(), String> {
    storage.store_peer_transport_keys(keys).await
}

/// Shared mutable state handed to `handle_message`. Every field has a
/// different owner; the struct only bundles the Arcs the peer reader task
/// already clones per connection.
#[derive(Clone)]
struct MessageContext {
    peers: Arc<Mutex<HashMap<usize, PeerState>>>,
    storage: Arc<dyn SyncRuntimeStorage>,
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

async fn save_removed_peer_ids(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_ids: &[String],
) -> Result<(), String> {
    storage.store_removed_peer_ids(peer_ids).await
}

fn send_msg(tx: &mpsc::UnboundedSender<Message>, msg: &LanSyncMessage) {
    let json = serialize_message(msg);
    let _ = tx.send(Message::Text(json.into()));
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
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    exclude_device_id: Option<&str>,
) {
    let peer_ids = {
        let peers_guard = peers.lock().await;
        peers_guard
            .iter()
            .filter(|(_, peer)| {
                peer.authenticated
                    && peer.sync_complete
                    && exclude_device_id != Some(peer.device_id.as_str())
            })
            .map(|(peer_id, _)| *peer_id)
            .collect::<Vec<_>>()
    };
    for peer_id in peer_ids {
        if let Err(error) = sync_server_peers::pump_committed_feed(storage, peers, peer_id).await {
            eprintln!("{TAG} committed feed wake failed: {error}");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
