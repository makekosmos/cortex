#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(
    all(windows, feature = "windows-gui-subsystem"),
    windows_subsystem = "windows"
)]

//! ark-core-rpc: stdin/stdout JSON-RPC binary used as a sidecar by Electron
//! and any other embedder that wants the full DB + sync runtime without
//! linking the Rust crate directly.
//!
//! Two stream shapes share stdout:
//!
//!   - **Response lines**: `{"ok": true|false, "data"?, "error"?}` — one per
//!     stdin request, emitted in request order.
//!   - **Event lines**: `{"event": "<kind>", ...}` — asynchronous broadcasts
//!     from the sync layer. Distinguishable from responses by the presence of
//!     an `event` field (and absence of `ok`). The embedder must demux.
//!
//! Request envelope (unchanged from the pre-sync binary):
//!   `{"operation": "<snake_case>", ...params}`
//!
//! New sync operations added in this file: `start_sync`, `stop_sync`,
//! `broadcast_change`, `get_connected_peers`, `leave_space`.

use std::collections::HashMap;
use std::io::Write as IoWrite;
use std::sync::{Arc, Mutex as StdMutex};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Mutex as TokioMutex};

use ark_core::beacon::{BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions};
use ark_core::db::{self, SqliteStorageBackend};
use ark_core::events::{emit_event, set_event_sender};
use ark_core::hlc::HLC;
use ark_core::host::{get_host_device_name, get_own_addresses};
use ark_core::integration_replication::{
    AuthorizedNode, IntegrationCredentialEnvelope, IntegrationNodeGrant, SignedSyncEnvelope,
};
use ark_core::net::is_address_routable;
use ark_core::protocol::LAN_SYNC_PORT;
use ark_core::relay_sync::{RelaySync, RelaySyncConfig};
use ark_core::sync_client::SyncClient;
use ark_core::sync_server::{StorageBackend, SyncServer};
use ark_core::transport_select::{select_transport, TransportChoice};
use ark_core::types::*;

include!("main/definitions.rs");
include!("main/runtime.rs");
include!("main/sync.rs");
include!("main/tests.rs");
