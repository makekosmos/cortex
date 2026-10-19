// ---------------------------------------------------------------------------
// Request envelope + sync-runtime state types. The former process-wide
// globals (DB / DB_PATH / SYNC / BACKUP_GATE) now live on `ServiceState`
// (see `service.rs`) so one process can host several services.
// ---------------------------------------------------------------------------

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SyncStartParams {
    pub(crate) space_id: String,
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) port: Option<u16>,
    pub(crate) seed_addresses: Option<Vec<String>>,
    pub(crate) relay_url: Option<String>,
    pub(crate) relay_api_key: Option<String>,
    pub(crate) auth_secret: Option<String>,
    pub(crate) use_iroh: bool,
    pub(crate) iroh_peer_ticket: Option<String>,
    /// Optional iroh relay server URL. `None` => `RelayMode::Default` (n0
    /// production relays) for the all-interfaces bind. A custom URL selects
    /// `RelayMode::Custom` — self-hosted relays and tests pointing the
    /// endpoint at an unreachable relay.
    pub(crate) iroh_relay_url: Option<String>,
    /// KOS-369: this start is the user entering a pairing code — mark the
    /// endpoint as an outgoing consent attempt. One-shot: the runtime's
    /// persisted `start_params` carry `false` so internal restarts and boot
    /// restores don't resurrect it.
    pub(crate) pairing_connect: bool,
    pub(crate) discovery_enabled: bool,
    pub(crate) bind: SyncBind,
    pub(crate) app_version: Option<String>,
}

fn default_discovery_enabled() -> bool {
    true
}

pub(crate) struct SyncRuntime {
    pub(crate) server: Arc<SyncServer>,
    pub(crate) storage: Arc<SqliteStorageBackend>,
    pub(crate) clients: Arc<TokioMutex<HashMap<String, Arc<DirectClient>>>>,
    pub(crate) relay: Option<Arc<RelaySync>>,
    pub(crate) transport_choice: Option<TransportChoice>,
    pub(crate) start_params: SyncStartParams,
    /// Step 4a: our iroh pairing ticket, resolved asynchronously after the
    /// iroh transport binds (`our_ticket()` needs the transport object
    /// directly — `RelaySync` only exposes `Arc<dyn SyncTransport>`, which
    /// is not downcastable — so the ticket string is published through this
    /// watch channel instead of threading a concrete `IrohTransport` handle
    /// through `SyncRuntime`). With relay homing enabled `our_ticket()`
    /// waits on the network (relay reachability), which must not stall the
    /// sequential request worker, so the start handler fills the channel
    /// from a spawned task. `borrow()` is `None` until the ticket is
    /// resolved, and stays `None` when iroh wasn't selected.
    pub(crate) iroh_our_ticket: watch::Receiver<Option<String>>,
    pub(crate) beacon: Arc<BroadcastDiscovery>,
    pub(crate) space_id: String,
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) auth_secret: Option<String>,
    pub(crate) own_addresses: Arc<TokioMutex<Vec<String>>>,
}

// The `SyncRuntime` slot lives on `ServiceState::sync` (`service.rs`).
// Events go out through `crate::events` — a process-wide broadcast bus also
// used by lib modules (notably `db::apply_entity_blocking` for schema-drift
// sync_error/sync_replay events).

// ---------------------------------------------------------------------------
// Request enum
// ---------------------------------------------------------------------------

pub(crate) use ark_domain::object_write::ObjectWriteSnapshot;

#[path = "definitions/request.rs"]
mod request;
pub(crate) use request::{DataVolumeScope, Request, SyncPolicyRuleInput};

include!("definitions/start_sync.rs");
