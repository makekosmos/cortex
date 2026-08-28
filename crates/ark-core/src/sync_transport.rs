//! Transport-neutral sync abstraction.
//!
//! `RelaySync` orchestration (CRDT version-vector exchange) is independent of
//! the underlying wire transport. This module defines the shared event enum
//! (`TransportEvent`) and the object-safe trait (`SyncTransport`) that both
//! `relay_transport::RelayTransport` and (behind `iroh-spike`)
//! `iroh_transport::IrohTransport` implement, so `RelaySync` can drive either
//! one via `Arc<dyn SyncTransport>`.

use tokio::sync::mpsc;

use crate::protocol::LanSyncMessage;

/// Lifecycle/message events emitted by a sync transport into the orchestration
/// layer. Structurally identical across transports.
#[derive(Debug)]
pub enum TransportEvent {
    Connected {
        device_id: String,
    },
    Disconnected {
        device_id: String,
    },
    MessageReceived {
        from_device_id: String,
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

    /// Signal the transport to stop.
    fn stop(&self);
}
