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
use crate::types::*;
use ark_sync::protocol::*;

pub use crate::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend,
    SyncRuntimeStorage,
};
pub use ark_application::sync_storage::SyncReadClass;

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
pub type OnRegistryEventsCallback =
    Arc<dyn Fn(Vec<ark_domain::device_registry::RegistryEvent>) + Send + Sync>;

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
    ready_peer: Option<ReadyPeer>,
    sync_started: bool,
    sync_complete: bool,
    negotiated_protocol: Option<ark_sync::protocol::NegotiatedProtocol>,
    registry_admission: Option<ark_sync::server_session::DirectServerAdmission>,
    in_flight_feed_position: Option<i64>,
    in_flight_feed_policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    pending_initial_snapshot_ack: Option<String>,
    pending_initial_snapshot_policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
    tx: mpsc::UnboundedSender<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadyPeer {
    connection_id: usize,
    device_id: String,
    registry_session_id: String,
}

impl ReadyPeer {
    fn new(connection_id: usize, device_id: String, registry_session_id: String) -> Option<Self> {
        (!device_id.is_empty() && !registry_session_id.is_empty()).then_some(Self {
            connection_id,
            device_id,
            registry_session_id,
        })
    }

    fn matches(&self, connection_id: usize, peer: &PeerState) -> bool {
        self.connection_id == connection_id
            && self.device_id == peer.device_id
            && peer
                .registry_admission
                .as_ref()
                .and_then(ark_sync::server_session::DirectServerAdmission::session_id)
                == Some(self.registry_session_id.as_str())
    }
}

async fn current_ready_peer(
    storage: &dyn SyncRuntimeStorage,
    peers: &Mutex<HashMap<usize, PeerState>>,
    connection_id: usize,
) -> Option<ReadyPeer> {
    let ready = {
        let peers = peers.lock().await;
        let peer = peers.get(&connection_id)?;
        let ready = peer.ready_peer.as_ref()?;
        ready.matches(connection_id, peer).then(|| ready.clone())?
    };
    let admitted = match storage.sync_session_is_admitted(&ready.device_id).await {
        Ok(admitted) => admitted,
        Err(error) => {
            eprintln!(
                "{TAG} direct session admission lookup failed for {}: {error}",
                ready.device_id
            );
            false
        }
    };
    if !admitted {
        let mut peers = peers.lock().await;
        if peers
            .get(&connection_id)
            .is_some_and(|peer| peer.ready_peer.as_ref() == Some(&ready))
        {
            if let Some(peer) = peers.get_mut(&connection_id) {
                peer.ready_peer = None;
                peer.sync_complete = false;
                peer.in_flight_feed_position = None;
                peer.pending_initial_snapshot_ack = None;
                let _ = peer.tx.send(Message::Close(None));
            }
        }
        return None;
    }
    let peers = peers.lock().await;
    let peer = peers.get(&connection_id)?;
    (peer.ready_peer.as_ref() == Some(&ready) && ready.matches(connection_id, peer))
        .then_some(ready)
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
    task_merge_bootstrap_v2: bool,
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
    on_registry_events: Arc<Mutex<Option<OnRegistryEventsCallback>>>,
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

pub(crate) async fn save_version_vector_with_policy(
    storage: &Arc<dyn SyncRuntimeStorage>,
    local_device_id: &str,
    policy_identity: &ark_application::sync_storage::PolicyIdentity,
    vector: &VersionVector,
) -> Result<(), String> {
    let mut merged = load_version_vector(storage).await?;
    merge_vector_entries(&mut merged, vector);
    storage
        .store_version_vector_with_policy(local_device_id, policy_identity, &merged)
        .await
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

pub(crate) async fn persist_pull_vector_with_policy(
    storage: &Arc<dyn SyncRuntimeStorage>,
    local_device_id: &str,
    policy_identity: &ark_application::sync_storage::PolicyIdentity,
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
    storage
        .store_version_vector_with_policy(local_device_id, policy_identity, vector)
        .await
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
    task_merge_bootstrap_v2: bool,
    known_peer_records: Arc<Mutex<Vec<PeerRecord>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_new_peer_discovered: Arc<Mutex<Option<OnNewPeerDiscoveredCallback>>>,
    on_registry_events: Arc<Mutex<Option<OnRegistryEventsCallback>>>,
}

async fn save_removed_peer_ids(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_ids: &[String],
) -> Result<(), String> {
    storage.store_removed_peer_ids(peer_ids).await
}

fn peer_message_allowed(
    ready_peer: Option<&ReadyPeer>,
    connection_id: usize,
    message: &LanSyncMessage,
) -> bool {
    ready_peer.is_some_and(|peer| peer.connection_id == connection_id)
        || !ark_sync::protocol::requires_registry_admission(message)
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
                peer.ready_peer.is_some()
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
        if let Err(error) = sync_server_peers::pump_task_merge_batch(storage, peers, peer_id).await
        {
            eprintln!("{TAG} task merge wake failed: {error}");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
