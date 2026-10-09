use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex};

use crate::hlc::HLC;
use crate::protocol::*;
use crate::relay_transport::{RelayConfig, RelayTransport};
use crate::sync_server::{
    OnChangeCallback, OnPeerConnectCallback, OnPeerDisconnectCallback, PeerEntry, StorageBackend,
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
    /// OS / product version the peer advertised in its Hello.
    platform: Option<String>,
    app_version: Option<String>,
    authenticated: bool,
}

struct IncomingSyncState {
    vector: VersionVector,
    changed: bool,
    last_update: Instant,
}

/// A user-initiated pairing action opened the first-contact window the
/// Hello gate consults: `show_pairing_code` admits any unknown endpoint
/// (`expected_endpoint: None` — whoever proves the ticket), while
/// `connect_with_pairing_code` pins the window to the ticketed endpoint.
/// Time-bounded because the Manager signals the card opening, never its
/// closing.
struct PairingAccept {
    expected_endpoint: Option<String>,
    until: Instant,
}

/// How long a pairing action keeps the first-contact window open. Covers
/// "the other device comes online a little late"; the dial loop retries
/// beyond it, but new unknown endpoints stop being admitted.
const PAIRING_ACCEPT_TTL: Duration = Duration::from_secs(300);

pub struct RelaySync {
    storage: Arc<dyn StorageBackend>,
    transport: Arc<dyn SyncTransport>,
    config: RelaySyncConfig,
    auth_secret: Option<String>,
    peers: Arc<Mutex<HashMap<String, RelayPeerState>>>,
    transport_keys: Arc<Mutex<HashMap<String, String>>>,
    pairing_accept: Mutex<Option<PairingAccept>>,
    incoming_sync: Arc<Mutex<Option<IncomingSyncState>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
    /// Background tasks spawned by `start()` — aborted and joined by
    /// `stop()` so a task holding `storage` (the shared db connection)
    /// cannot outlive teardown (KOS-270).
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
