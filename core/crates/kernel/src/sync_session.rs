use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use crate::replication_storage::SyncRuntimeStorage;
use crate::sync_server::{
    OnChangeCallback, OnPeerConnectCallback, OnPeerDisconnectCallback, PeerEntry,
};
use crate::sync_transport::{SyncTransport, TransportEvent};
use crate::types::SyncEntity;
use ::domain::sync_contracts::protocol_negotiation::*;
use ::domain::sync_contracts::wire::*;
use ::net::relay::{RelayConfig, RelayTransport};
// Shared with lifecycle's existing parent-scope imports.
pub(super) use std::time::Duration;

mod lifecycle;
mod registry_exchange;
mod registry_fanout;
mod task_merge;
mod transport_events;

#[cfg(test)]
mod pairing_retry_tests;

#[cfg(test)]
mod revoked_device_catchup_tests;

#[cfg(test)]
mod transport_events_tests;

mod disconnect;
mod helpers;
mod inbound;
mod outbound;
mod pairing;
mod types;

use helpers::{apply_sync_entity, load_version_vector, trim_process_heap};
use types::{
    IncomingSyncState, OutgoingPairing, PendingPairingRequest, SyncPeerState,
    PAIRING_DECISION_TIMEOUT, PAIRING_DECLINE_SUPPRESS, SYNC_LOAD_PAGE_SIZE, TAG,
};
pub use types::{
    OnPairingChangedCallback, OnRegistryEventsCallback, SyncSession, SyncSessionConfig,
};
