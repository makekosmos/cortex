use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use crate::replication_storage::SyncRuntimeStorage;
use crate::sync_server::{
    OnChangeCallback, OnPeerConnectCallback, OnPeerDisconnectCallback, PeerEntry,
};
use crate::sync_transport::{SyncTransport, TransportEvent};
use crate::types::SyncEntity;
use ark_domain::sync_contracts::protocol_negotiation::*;
use ark_domain::sync_contracts::wire::*;
use ark_net::relay::{RelayConfig, RelayTransport};
// Shared with lifecycle's existing parent-scope imports.
pub(super) use std::time::Duration;

mod lifecycle;
mod registry_exchange;
mod registry_fanout;
mod task_merge;
mod transport_events;

#[cfg(test)]
#[path = "relay_sync/pairing_retry_tests.rs"]
mod pairing_retry_tests;

#[cfg(test)]
#[path = "relay_sync/revoked_device_catchup_tests.rs"]
mod revoked_device_catchup_tests;

#[cfg(test)]
#[path = "relay_sync/transport_events_tests.rs"]
mod transport_events_tests;

#[path = "relay_disconnect.rs"]
mod disconnect;
#[path = "relay_helpers.rs"]
mod helpers;
#[path = "relay_impl_1.rs"]
mod impl_1;
#[path = "relay_impl_2.rs"]
mod impl_2;
#[path = "relay_pairing.rs"]
mod pairing;
#[path = "relay_types.rs"]
mod types;

use helpers::{apply_relay_entity, load_version_vector, trim_process_heap};
use types::{
    IncomingSyncState, OutgoingPairing, PendingPairingRequest, RelayPeerState,
    PAIRING_DECISION_TIMEOUT, PAIRING_DECLINE_SUPPRESS, SYNC_LOAD_PAGE_SIZE, TAG,
};
pub use types::{OnPairingChangedCallback, OnRegistryEventsCallback, RelaySync, RelaySyncConfig};
