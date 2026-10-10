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

/// A first-contact Hello parked for explicit user consent (KOS-369). The
/// stored Hello is replayed through `handle_message` on «Принять», so
/// acceptance applies exactly the same checks and persistence as a live
/// Hello; «Отклонить» answers `PairingRejected` and closes the connection.
struct PendingPairingRequest {
    device_name: String,
    platform: Option<String>,
    app_version: Option<String>,
    transport_public_key: String,
    hello: LanSyncMessage,
}

/// Our own outbound pairing attempt (this device entered a code). The
/// endpoint id ties consent to the ticketed device — its Hello is
/// pre-consented by the user's own «Подключить».
#[derive(Clone)]
struct OutgoingPairing {
    endpoint: String,
    started_at: Instant,
    declined: bool,
    /// The responder actually consented: it sent a second Hello (the accept
    /// broadcast) or data. The conn-setup Hello alone proves nothing —
    /// listeners inject it before a human decides.
    accepted: bool,
}

/// How long the initiator's snapshot keeps reporting `pending` before the
/// Manager calls it a timeout. The dial itself keeps retrying — a late
/// «Принять» still completes the pairing; the bound is purely for the UI.
const PAIRING_DECISION_TIMEOUT: Duration = Duration::from_secs(120);

/// After «Отклонить» the same device id's Hellos are dropped without a new
/// prompt for this long — old-version initiators keep redialing and would
/// otherwise resurrect the dialog every backoff tick.
const PAIRING_DECLINE_SUPPRESS: Duration = Duration::from_secs(120);

pub type OnPairingChangedCallback = Arc<dyn Fn() + Send + Sync>;

pub struct RelaySync {
    storage: Arc<dyn StorageBackend>,
    transport: Arc<dyn SyncTransport>,
    config: RelaySyncConfig,
    auth_secret: Option<String>,
    peers: Arc<Mutex<HashMap<String, RelayPeerState>>>,
    transport_keys: Arc<Mutex<HashMap<String, String>>>,
    /// First-contact Hellos awaiting «Принять / Отклонить», keyed by the
    /// claimed device id.
    pending_pairing: Mutex<HashMap<String, PendingPairingRequest>>,
    /// Recently declined device ids — their Hellos are dropped without
    /// re-prompting until the suppression lapses.
    declined_pairing: Mutex<HashMap<String, Instant>>,
    /// Our outbound attempt after «Подключить» — consents the ticketed
    /// endpoint's Hello and reports the outcome to the snapshot.
    outgoing_pairing: Mutex<Option<OutgoingPairing>>,
    incoming_sync: Arc<Mutex<Option<IncomingSyncState>>>,
    on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
    on_pairing_changed: Arc<Mutex<Option<OnPairingChangedCallback>>>,
    /// Background tasks spawned by `start()` — aborted and joined by
    /// `stop()` so a task holding `storage` (the shared db connection)
    /// cannot outlive teardown (KOS-270).
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
