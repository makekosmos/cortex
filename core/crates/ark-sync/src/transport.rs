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
use ark_application::replication_storage::IntegrationReplicationBackend;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressedSendOutcome {
    Sent,
    PeerOffline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerDisconnectOutcome {
    PeerClosed,
    AlreadyOffline,
    SharedRelayUnaffected,
}

/// Narrow capability used by an addressed transport immediately before it
/// writes a signed integration frame. The implementation owns the policy and
/// persistence access; transports only provide the live recipient key.
#[async_trait::async_trait]
pub trait OutboundIntegrationFrameAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        frame: &crate::SignedSyncEnvelope,
        transport_public_key: &str,
    ) -> Result<(), String>;
}

pub struct StorageBackedOutboundIntegrationFrameAuthorizer {
    storage: std::sync::Arc<dyn IntegrationReplicationBackend>,
    expected_space_id: String,
    expected_origin_node_id: String,
}

impl StorageBackedOutboundIntegrationFrameAuthorizer {
    pub fn new(
        storage: std::sync::Arc<dyn IntegrationReplicationBackend>,
        expected_space_id: impl Into<String>,
        expected_origin_node_id: impl Into<String>,
    ) -> Self {
        Self {
            storage,
            expected_space_id: expected_space_id.into(),
            expected_origin_node_id: expected_origin_node_id.into(),
        }
    }
}

#[async_trait::async_trait]
impl OutboundIntegrationFrameAuthorizer for StorageBackedOutboundIntegrationFrameAuthorizer {
    async fn authorize(
        &self,
        frame: &crate::SignedSyncEnvelope,
        transport_public_key: &str,
    ) -> Result<(), String> {
        self.storage
            .validate_outbound_signed_integration_frame_with_transport(
                frame,
                &self.expected_space_id,
                &self.expected_origin_node_id,
                transport_public_key,
            )
            .await
    }
}

/// Shared lifecycle and broadcast surface implemented by every sync transport.
///
/// Uses `async_trait` to keep the transport usable behind `Arc<dyn SyncTransport>`.
#[async_trait::async_trait]
pub trait SyncTransport: Send + Sync {
    /// Start the transport. Returns immediately; background tasks drive the
    /// connection and forward events through `event_tx`.
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String>;

    /// Enqueue a message for sending.
    fn send(&self, msg: LanSyncMessage) -> Result<(), String>;

    /// Stop the transport and join its background tasks. Must not return
    /// while a spawned task is still running: tasks hold transport callbacks
    /// until teardown completes.
    async fn stop(&self);
}

/// Capability contract for a transport that supports Iroh-style peer sessions.
/// Implementing this trait declares that all of its required per-peer
/// operations are available; callers should use the trait bound or a typed
/// transport variant instead of probing for capability at runtime. Relay
/// intentionally does not implement this contract because its shared
/// WebSocket cannot admit, address, or disconnect a single remote peer.
///
/// This remains object-safe so orchestration can use
/// `Arc<dyn IrohSessionTransport>` when it has selected an Iroh session.
#[async_trait::async_trait]
pub trait IrohSessionTransport: SyncTransport {
    async fn send_to(&self, device_id: &str, msg: LanSyncMessage) -> Result<(), String>;

    fn set_outbound_authorizer(
        &self,
        authorizer: Arc<dyn OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String>;

    /// Bind the verified endpoint for control traffic. Implementations must
    /// keep user-data frames blocked until `mark_peer_data_ready` succeeds.
    fn bind_peer_for_control(
        &self,
        device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String>;

    /// Promote a verified transport binding after durable peer enrollment.
    /// Data frames remain blocked until this succeeds.
    fn mark_peer_data_ready(
        &self,
        device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String>;

    /// Drop a binding made during a handshake or failed admission.
    fn unbind_peer(&self, device_id: &str) -> Result<(), String>;

    /// Addressed send to a connection identified by its transport key —
    /// before data admission, for pairing and registry control messages.
    async fn send_to_transport_peer(
        &self,
        transport_public_key: &str,
        msg: LanSyncMessage,
    ) -> Result<AddressedSendOutcome, String>;

    fn disconnect_peer(&self, device_id: &str) -> Result<PeerDisconnectOutcome, String>;

    fn disconnect_transport_peer(
        &self,
        transport_public_key: &str,
    ) -> Result<PeerDisconnectOutcome, String>;
}
