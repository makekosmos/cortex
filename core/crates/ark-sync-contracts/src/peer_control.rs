use crate::transport::PeerDisconnectOutcome;
use crate::wire::LanSyncMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerControlSendOutcome {
    Sent,
    PeerOffline,
}

#[async_trait::async_trait]
pub trait PeerSessionControl: Send + Sync {
    async fn send_control_to_endpoint(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<PeerControlSendOutcome, String>;
    fn mark_peer_data_ready(&self, peer: &str, endpoint: &str) -> Result<(), String>;
    fn unbind_peer(&self, peer: &str) -> Result<(), String>;
    fn disconnect_peer(&self, peer: &str) -> Result<PeerDisconnectOutcome, String>;
    fn disconnect_transport_endpoint(
        &self,
        endpoint: &str,
    ) -> Result<PeerDisconnectOutcome, String>;
}
