use crate::wire::LanSyncMessage;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum TransportEvent {
    Connected {
        device_id: String,
    },
    Disconnected {
        device_id: String,
    },
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

#[async_trait::async_trait]
pub trait OutboundIntegrationFrameAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        frame: &ark_domain::integration_replication::SignedSyncEnvelope,
        transport_public_key: &str,
    ) -> Result<(), String>;
}

#[async_trait::async_trait]
pub trait SyncTransport: Send + Sync {
    async fn start(&self, event_tx: mpsc::UnboundedSender<TransportEvent>) -> Result<(), String>;
    fn send(&self, msg: LanSyncMessage) -> Result<(), String>;
    async fn stop(&self);
}

#[async_trait::async_trait]
pub trait IrohSessionTransport: SyncTransport {
    async fn send_to(&self, device_id: &str, msg: LanSyncMessage) -> Result<(), String>;
    fn set_outbound_authorizer(
        &self,
        authorizer: Arc<dyn OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String>;
    fn bind_peer_for_control(
        &self,
        device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String>;
    fn mark_peer_data_ready(
        &self,
        device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String>;
    fn unbind_peer(&self, device_id: &str) -> Result<(), String>;
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
