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
use crate::sync_server::StorageBackend;

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
/// Uses `async_trait` to match the crate's established async-trait pattern
/// (`StorageBackend` in `sync_server.rs`), keeping the trait usable behind
/// `Arc<dyn SyncTransport>`.
#[async_trait::async_trait]
pub trait SyncTransport: Send + Sync {
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
        _storage: Arc<dyn StorageBackend>,
        _space_id: &str,
        _origin_node_id: &str,
    ) -> Result<(), String> {
        Err("outbound authorization is not supported".into())
    }

    fn bind_authenticated_peer(
        &self,
        _device_id: &str,
        _transport_public_key: &str,
    ) -> Result<(), String> {
        Err("authenticated transport binding is not supported".into())
    }

    /// Drop a binding made by `bind_authenticated_peer` when the handshake
    /// it enabled fails afterwards (HMAC/protocol/removed-peer rejection).
    fn unbind_authenticated_peer(&self, _device_id: &str) -> Result<(), String> {
        Err("authenticated transport binding is not supported".into())
    }

    /// Addressed send to a connection identified by its transport key —
    /// before authentication, for handshake-level replies only
    /// (`PairingRejected`; `bind_authenticated_peer` has not run yet, so
    /// `send_to`'s authenticated lookup cannot reach the peer).
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

    /// Signal the transport to stop.
    fn stop(&self);
}
