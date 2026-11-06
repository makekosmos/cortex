use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::replication_storage::SyncRuntimeStorage;
use crate::sync_server::{OnChangeCallback, OnPeerConnectCallback, OnPeerDisconnectCallback};
use crate::sync_transport::SyncTransportHandle;
use crate::types::VersionVector;
use ark_domain::sync_contracts::wire::LanSyncMessage;

pub(super) const TAG: &str = "[RelaySync]";
pub(super) const SYNC_LOAD_PAGE_SIZE: usize = 100;

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
pub(super) struct RelayPeerState {
    pub(super) device_name: String,
    /// OS / product version the peer advertised in its Hello.
    pub(super) platform: Option<String>,
    pub(super) app_version: Option<String>,
    pub(super) authenticated: bool,
    pub(super) negotiated_protocol:
        Option<ark_domain::sync_contracts::protocol_negotiation::NegotiatedProtocol>,
}

pub(super) struct IncomingSyncState {
    pub(super) vector: VersionVector,
    pub(super) changed: bool,
    pub(super) last_update: Instant,
}

/// A first-contact Hello parked for explicit user consent (KOS-369). The
/// stored Hello is replayed through `handle_message` on «Принять», so
/// acceptance applies exactly the same checks and persistence as a live
/// Hello; «Отклонить» answers `PairingRejected` and closes the connection.
pub(super) struct PendingPairingRequest {
    pub(super) device_name: String,
    pub(super) platform: Option<String>,
    pub(super) app_version: Option<String>,
    pub(super) transport_public_key: String,
    pub(super) hello: LanSyncMessage,
}

#[derive(Clone)]
pub(super) struct OutgoingPairing {
    pub(super) endpoint: String,
    pub(super) started_at: Instant,
    pub(super) declined: bool,
    pub(super) accepted: bool,
}

/// How long the initiator's snapshot keeps reporting `pending` before the
/// Manager calls it a timeout. The dial itself keeps retrying — a late
/// «Принять» still completes the pairing; the bound is purely for the UI.
pub(super) const PAIRING_DECISION_TIMEOUT: Duration = Duration::from_secs(120);

/// After «Отклонить» the same device id's Hellos are dropped without a new
/// prompt for this long — old-version initiators keep redialing and would
/// otherwise resurrect the dialog every backoff tick.
pub(super) const PAIRING_DECLINE_SUPPRESS: Duration = Duration::from_secs(120);

pub type OnPairingChangedCallback = Arc<dyn Fn() + Send + Sync>;
pub type OnRegistryEventsCallback =
    Arc<dyn Fn(Vec<ark_domain::device_registry::RegistryEvent>) + Send + Sync>;

pub struct RelaySync {
    pub(super) storage: Arc<dyn SyncRuntimeStorage>,
    pub(super) transport: SyncTransportHandle,
    pub(super) registry:
        Option<ark_sync::relay_registry::RelayRegistryCoordinator<dyn SyncRuntimeStorage>>,
    pub(super) config: RelaySyncConfig,
    pub(super) auth_secret: Option<String>,
    pub(super) peers: Arc<Mutex<HashMap<String, RelayPeerState>>>,
    pub(super) transport_keys: Arc<Mutex<HashMap<String, String>>>,
    /// First-contact Hellos awaiting «Принять / Отклонить», keyed by the
    /// claimed device id.
    pub(super) pending_pairing: Mutex<HashMap<String, PendingPairingRequest>>,
    pub(super) pending_responder_hellos: Mutex<HashMap<String, LanSyncMessage>>,
    /// Recently declined device ids — their Hellos are dropped without
    /// re-prompting until the suppression lapses.
    pub(super) declined_pairing: Mutex<HashMap<String, Instant>>,
    /// Our outbound attempt after «Подключить» — consents the ticketed
    /// endpoint's Hello and reports the outcome to the snapshot.
    pub(super) outgoing_pairing: Mutex<Option<OutgoingPairing>>,
    pub(super) incoming_sync: Arc<Mutex<Option<IncomingSyncState>>>,
    pub(super) on_change: Arc<Mutex<Option<OnChangeCallback>>>,
    pub(super) on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    pub(super) on_peer_disconnect: Arc<Mutex<Option<OnPeerDisconnectCallback>>>,
    pub(super) on_pairing_changed: Arc<Mutex<Option<OnPairingChangedCallback>>>,
    pub(super) on_registry_events: Arc<Mutex<Option<OnRegistryEventsCallback>>>,
    /// Background tasks spawned by `start()` — aborted and joined by
    /// `stop()` so a task holding `storage` (the shared db connection)
    /// cannot outlive teardown (KOS-270).
    pub(super) tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
