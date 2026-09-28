//! UniFFI-exposed facade used by Android (and any future Kotlin/Swift
//! embedders). Wraps the same DB + sync runtime the in-process service
//! drives, plus a callback-interface listener for async events.
//!
//! Design notes:
//!   - Each UniFFI-exposed struct holds its own tokio runtime. Callbacks run
//!     on a dedicated thread so they can invoke the Kotlin listener without
//!     blocking the sync engine.
//!   - Errors remain `ArkCoreError::Generic(String)` for wire compatibility,
//!     but compatibility failures use a stable JSON category/code/pointer
//!     payload rather than free-form strings.
//!   - Types that cross the FFI boundary are plain JSON strings for
//!     entities and simple owned strings/ints everywhere else. UniFFI
//!     callback interfaces only support primitive / String / owned struct
//!     parameters, so the `ArkEventListener` trait carries JSON payloads
//!     for entity changes to keep the FFI surface stable across protocol
//!     tweaks.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use rusqlite::Connection;
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use tokio::sync::{Mutex as TokioMutex, RwLock};

/// Shutdown signal sender kept alive while sync is running.
type ShutdownTx = tokio::sync::oneshot::Sender<()>;

use crate::beacon::{BeaconPeer, BroadcastDiscovery, BroadcastDiscoveryOptions};
use crate::db::{
    clear_all as db_clear_all, delete_tracked_app as db_delete_tracked_app,
    delete_trashed as db_delete_trashed, delete_usage_event as db_delete_usage_event,
    delete_usage_session as db_delete_usage_session, get_sync_kv as db_get_kv, init_schema,
    load_all as db_load_all, open_db, set_sync_kv as db_set_kv,
    upsert_tracked_app as db_upsert_tracked_app, upsert_usage_event as db_upsert_usage_event,
    upsert_usage_session as db_upsert_usage_session, SqliteStorageBackend,
};
use crate::host::{get_host_device_name, get_own_addresses};
use crate::net::is_address_routable;
use crate::protocol::LAN_SYNC_PORT;
use crate::relay_sync::{RelaySync, RelaySyncConfig};
use crate::sync_client::SyncClient;
use crate::sync_server::{StorageBackend, SyncServer};
use crate::types::{
    LoadAllData, PeerRecord, Project, SyncEntity, Tag, TodoItem, TrackedApp, UsageEvent,
    UsageSession,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ArkCoreError {
    #[error("{0}")]
    Generic(String),
}

impl From<String> for ArkCoreError {
    fn from(value: String) -> Self {
        Self::Generic(value)
    }
}

impl From<&str> for ArkCoreError {
    fn from(value: &str) -> Self {
        Self::Generic(value.to_string())
    }
}

type Result<T> = std::result::Result<T, ArkCoreError>;

fn err<S: Into<String>>(msg: S) -> ArkCoreError {
    ArkCoreError::Generic(msg.into())
}

fn structured_error(category: &str, code: &str, pointer: Option<&str>) -> ArkCoreError {
    err(serde_json::json!({
        "category": category,
        "code": code,
        "pointer": pointer,
    })
    .to_string())
}

fn compatibility_error<E: std::fmt::Display>(message: E) -> ArkCoreError {
    let message = message.to_string();
    let mut parts = message.splitn(3, ':');
    let code = parts.next().unwrap_or("storage");
    let source_id = parts.next().unwrap_or("");
    let pointer = parts.next().unwrap_or("");
    err(serde_json::json!({
        "category": "compatibility",
        "code": code,
        "pointer": pointer,
        "sourceKind": "ffi",
        "sourceId": source_id,
    })
    .to_string())
}

fn malformed_json_error(pointer: &'static str) -> ArkCoreError {
    structured_error("invalid_request", "malformed_json", Some(pointer))
}

fn ingress_error(error: crate::canonical_types::ingress::CanonicalIngressError) -> ArkCoreError {
    if error
        .pointer
        .as_deref()
        .is_some_and(|pointer| pointer.starts_with("/content"))
    {
        return structured_error("compatibility", "DATA_LOSS_RISK", error.pointer.as_deref());
    }
    structured_error(error.category, error.code, error.pointer.as_deref())
}

// ---------------------------------------------------------------------------
// Plain records that cross the FFI boundary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiConnectedPeer {
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSyncConfig {
    pub space_id: String,
    pub device_id: String,
    pub device_name: Option<String>,
    pub port: Option<u32>,
    pub db_path: Option<String>,
    pub seed_addresses: Vec<String>,
    /// Optional relay WebSocket URL (e.g. "wss://relay.example.com").
    /// Defaults to None — existing callers need no changes.
    #[uniffi(default = None)]
    pub relay_url: Option<String>,
    /// API key for the relay server. Required when relay_url is Some.
    #[uniffi(default = None)]
    pub relay_api_key: Option<String>,
    /// Optional shared secret for LAN/P2P hello HMAC authentication.
    #[uniffi(default = None)]
    pub auth_secret: Option<String>,
    /// Step 4a: select the iroh p2p transport instead of relay. Present on
    /// the UniFFI surface in every build (stable struct shape); only acted
    /// on behind `#[cfg(feature = "iroh-spike")]` in `start_sync` — a build
    /// without the feature returns an error if this is `true` instead of
    /// silently falling back to relay/LAN.
    #[uniffi(default = false)]
    pub use_iroh: bool,
    /// Pairing ticket string for the iroh peer (see
    /// `iroh_transport::IrohTransport::our_ticket`/`from_ticket`).
    #[uniffi(default = None)]
    pub iroh_peer_ticket: Option<String>,
}

// ---------------------------------------------------------------------------
// Callback interface — Kotlin implements this and passes it at start_sync
// ---------------------------------------------------------------------------

#[uniffi::export(callback_interface)]
pub trait ArkEventListener: Send + Sync {
    /// Called once for every inbound entity change (sync batch or live).
    /// `entity_json` is the serialised `SyncEntity` (snake_case protocol form)
    /// so the Kotlin side can parse it without re-exporting the full struct.
    fn on_entity_changed(&self, entity_json: String);

    /// A new peer authenticated. `device_name` may be empty if the peer did
    /// not advertise a name.
    fn on_peer_connected(&self, device_id: String, device_name: String);

    /// A previously-authenticated peer disconnected.
    fn on_peer_disconnected(&self, device_id: String, remaining: u32);
}

// ---------------------------------------------------------------------------
// ArkCore — the single FFI-exposed object
// ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct ArkCore {
    runtime: Runtime,
    db: StdMutex<Option<Arc<StdMutex<Connection>>>>,
    /// Shutdown channel for the background sync thread.
    sync_shutdown: StdMutex<Option<ShutdownTx>>,
    /// Lightweight mirror of the running sync context for `broadcast_change_json` / `get_connected_peers`.
    sync: TokioMutex<Option<SyncRuntime>>,
    listener: RwLock<Option<Arc<dyn ArkEventListener>>>,
}

struct SyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    relay: Option<Arc<RelaySync>>,
    /// Step 4a: our iroh pairing ticket, captured when the iroh transport
    /// was selected (mirrors `main.rs::SyncRuntime::iroh_our_ticket`).
    iroh_our_ticket: Option<String>,
    beacon: Arc<BroadcastDiscovery>,
    device_id: String,
    device_name: String,
    space_id: String,
    auth_secret: Option<String>,
    own_addresses: Vec<String>,
}

#[path = "ffi_callbacks.rs"]
mod ffi_callbacks;
#[path = "ffi_control.rs"]
mod ffi_control;
#[path = "ffi_database.rs"]
mod ffi_database;
#[path = "ffi_integration.rs"]
mod ffi_integration;
#[path = "ffi_legacy.rs"]
mod ffi_legacy;
#[path = "ffi_live.rs"]
mod ffi_live;
#[path = "ffi_start.rs"]
mod ffi_start;
#[path = "ffi_sync_clients.rs"]
mod ffi_sync_clients;
#[path = "ffi_sync_state.rs"]
mod ffi_sync_state;

struct CloneSyncRuntime {
    server: Arc<SyncServer>,
    storage: Arc<SqliteStorageBackend>,
    clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    relay: Option<Arc<RelaySync>>,
    device_id: String,
    device_name: String,
    space_id: String,
    auth_secret: Option<String>,
    own_addresses: Vec<String>,
}

// Silence unused warnings: the same CloneSyncRuntime is used only for the
// public helpers above; add a dummy accessor so rustc doesn't complain about
// `storage`/`clients` being unreachable through the field syntax.
#[allow(dead_code)]
fn _touch(r: &CloneSyncRuntime) -> Value {
    Value::String(format!("{} {} {}", r.device_id, r.device_name, r.space_id))
}

// ---------------------------------------------------------------------------
// UniFFI-exposed free functions (mirror main.rs convenience helpers)
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn ffi_host_device_name() -> String {
    get_host_device_name()
}

#[uniffi::export]
pub fn ffi_own_addresses(port: u32) -> Vec<String> {
    get_own_addresses(port as u16)
}
