use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;
#[cfg(test)]
use tokio_tungstenite::tungstenite::Message;

use crate::hlc::HLC;
use crate::integration_replication::SignedSyncEnvelope;
use crate::types::*;
use ark_sync::protocol::merge_peer_records;
use ark_sync_contracts::protocol_negotiation::normalize_auth_secret;
use ark_sync_contracts::wire::*;

pub use crate::replication_storage::{
    AuthenticatedEntityBackend, IntegrationReplicationBackend, PeerIdentityBackend,
    SyncRuntimeStorage,
};
pub use ark_application::sync_storage::SyncReadClass;

#[cfg(test)]
#[path = "sync_server/sync_round_trip.rs"]
mod sync_round_trip_tests;
#[path = "sync_server_core.rs"]
mod sync_server_core;
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
    session: ark_sync::server_session::ServerSessionHandle,
    tx: PeerMessageSender,
}

#[cfg(test)]
pub(crate) fn test_server_session_handle(
    device_id: &str,
) -> ark_sync::server_session::ServerSessionHandle {
    test_server_session_with_keepalive(device_id).0
}

#[cfg(test)]
pub(crate) fn test_server_session_with_keepalive(
    device_id: &str,
) -> (ark_sync::server_session::ServerSessionHandle, impl Send) {
    ark_sync::server_session::test_ready_server_session(device_id)
        .expect("session identity is available")
}

#[derive(Clone)]
enum PeerMessageSender {
    Session(ark_sync_contracts::session_io::SessionIoSender),
    #[cfg(test)]
    Test(mpsc::UnboundedSender<Message>),
}

impl PeerMessageSender {
    fn session(session: ark_sync_contracts::session_io::SessionIoSender) -> Self {
        Self::Session(session)
    }

    #[cfg(test)]
    pub(crate) fn test(sender: mpsc::UnboundedSender<Message>) -> Self {
        Self::Test(sender)
    }

    fn try_send(&self, message: LanSyncMessage) -> Result<(), ()> {
        match self {
            Self::Session(session) => session.try_send(message).map_err(|_| ()),
            #[cfg(test)]
            Self::Test(test) => {
                let text = serialize_message(&message);
                test.send(Message::Text(text.into())).map_err(|_| ())
            }
        }
    }

    fn close(&self) {
        match self {
            Self::Session(session) => session.close(),
            #[cfg(test)]
            Self::Test(test) => {
                let _ = test.send(Message::Close(None));
            }
        }
    }

    fn close_after_flush(&self) {
        match self {
            Self::Session(session) => session.close_after_flush(),
            #[cfg(test)]
            Self::Test(test) => {
                let _ = test.send(Message::Close(None));
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadyPeer {
    connection_id: usize,
    device_id: String,
    registry_session_id: String,
}

impl ReadyPeer {
    fn matches(&self, connection_id: usize, peer: &PeerState) -> bool {
        let projection = peer.session.projection();
        self.connection_id == connection_id
            && peer.session.control() == ark_sync::server_session::ServerSessionControl::Active
            && projection.readiness == ark_sync::server_session::ServerPeerReadiness::Ready
            && projection.device_id.as_deref() == Some(self.device_id.as_str())
            && projection.registry_session_id.as_deref() == Some(self.registry_session_id.as_str())
    }
}

impl PeerState {
    fn is_ready(&self) -> bool {
        self.session.projection().readiness == ark_sync::server_session::ServerPeerReadiness::Ready
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
        let projection = peer.session.projection();
        if projection.readiness != ark_sync::server_session::ServerPeerReadiness::Ready {
            return None;
        }
        let ready = ReadyPeer {
            connection_id,
            device_id: projection.device_id?,
            registry_session_id: projection.registry_session_id?,
        };
        ready.matches(connection_id, peer).then_some(ready)?
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
        if peers.get(&connection_id).is_some_and(|peer| {
            peer.session.projection().readiness
                == ark_sync::server_session::ServerPeerReadiness::Ready
                && peer.session.projection().device_id.as_deref() == Some(ready.device_id.as_str())
                && peer.session.projection().registry_session_id.as_deref()
                    == Some(ready.registry_session_id.as_str())
        }) {
            if let Some(peer) = peers.get_mut(&connection_id) {
                peer.session.revoke();
                peer.tx.close();
            }
        }
        return None;
    }
    let peers = peers.lock().await;
    let peer = peers.get(&connection_id)?;
    (ready.matches(connection_id, peer)).then_some(ready)
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
    /// Per-connection protocol handlers, joined by `stop()`.
    conn_tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// WebSocket supervisors are tracked separately so `stop()` can signal
    /// them through `SessionIo` before joining them, even after handler abort.
    transport_tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
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
    storage.merge_version_vector(vector).await
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

async fn save_removed_peer_ids(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peer_ids: &[String],
) -> Result<(), String> {
    storage.store_removed_peer_ids(peer_ids).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
