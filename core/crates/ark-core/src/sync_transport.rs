//! Transport-neutral sync abstraction.
//!
//! `RelaySync` orchestration (CRDT version-vector exchange) is independent of
//! the underlying wire transport. This module defines the shared event enum
//! (`TransportEvent`) and the object-safe trait (`SyncTransport`) that both
//! `relay_transport::RelayTransport` and `iroh_transport::IrohTransport`
//! implement, so `RelaySync` can drive either
//! one via `Arc<dyn SyncTransport>`.

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::protocol::LanSyncMessage;
use crate::replication_storage::SyncRuntimeStorage;

/// Lifecycle/message events emitted by a sync transport into the orchestration
/// layer. Structurally identical across transports.
#[derive(Debug)]
pub enum TransportEvent {
    Connected {
        device_id: String,
    },
    /// A single remote peer disconnected. `device_id` is the REMOTE peer.
    Disconnected {
        device_id: String,
    },
    /// The whole transport connection dropped (e.g. the relay WebSocket was
    /// lost). Every remote peer reached through it is now unreachable; no
    /// single peer identity applies.
    TransportDropped,
    MessageReceived {
        from_device_id: String,
        msg: LanSyncMessage,
    },
    MessageReceivedFromTransport {
        from_device_id: String,
        transport_public_key: String,
        msg: LanSyncMessage,
    },
}

/// Object-safe transport abstraction consumed by `RelaySync`.
///
/// Uses `async_trait` to keep the transport usable behind `Arc<dyn SyncTransport>`.
#[async_trait::async_trait]
pub trait SyncTransport: Send + Sync {
    /// True only when the transport enforces pre-admission control and can
    /// exchange signed registry history before carrying user data.
    fn supports_registry_admission(&self) -> bool {
        false
    }

    /// Start the transport. Returns immediately; background tasks drive the
    /// connection and forward events through `event_tx`.
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String>;

    /// Enqueue a message for sending.
    fn send(&self, msg: LanSyncMessage) -> Result<(), String>;

    async fn send_to(&self, _device_id: &str, _msg: LanSyncMessage) -> Result<(), String> {
        Err("addressed transport is not supported".into())
    }

    fn set_outbound_storage(
        &self,
        _storage: Arc<dyn SyncRuntimeStorage>,
        _space_id: &str,
        _origin_node_id: &str,
    ) -> Result<(), String> {
        Err("outbound authorization is not supported".into())
    }

    /// Bind the verified endpoint for control traffic. Implementations must
    /// keep user-data frames blocked until `mark_peer_data_ready` succeeds.
    fn bind_peer_for_control(
        &self,
        _device_id: &str,
        _transport_public_key: &str,
    ) -> Result<(), String> {
        Err("authenticated transport binding is not supported".into())
    }

    /// Promote a verified transport binding after durable peer enrollment.
    /// Data frames remain blocked until this succeeds.
    fn mark_peer_data_ready(
        &self,
        _device_id: &str,
        _transport_public_key: &str,
    ) -> Result<(), String> {
        Err("transport data admission is not supported".into())
    }

    /// Drop a binding made during a handshake or failed admission.
    fn unbind_peer(&self, _device_id: &str) -> Result<(), String> {
        Err("authenticated transport binding is not supported".into())
    }

    /// Addressed send to a connection identified by its transport key —
    /// before data admission, for pairing and registry control messages.
    async fn send_to_transport_peer(
        &self,
        _transport_public_key: &str,
        _msg: LanSyncMessage,
    ) -> Result<(), String> {
        Err("transport-key addressed send is not supported".into())
    }

    fn disconnect_peer(&self, _device_id: &str) -> Result<(), String> {
        Err("addressed peer disconnect is not supported".into())
    }

    fn disconnect_transport_peer(&self, _transport_public_key: &str) -> Result<(), String> {
        Err("transport peer disconnect is not supported".into())
    }

    /// Stop the transport and join its background tasks. Must not return
    /// while a spawned task is still running: tasks hold `outbound_storage`
    /// (the open `ark.db` connection), and a task that outlives `stop()`
    /// keeps the database file open past teardown — on Windows the
    /// containing directory then cannot be deleted (KOS-369).
    async fn stop(&self);
}
