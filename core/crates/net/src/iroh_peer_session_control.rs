use domain::sync_contracts::wire::LanSyncMessage;

use super::transport::IrohTransport;

#[async_trait::async_trait]
impl domain::sync_contracts::peer_control::PeerSessionControl for IrohTransport {
    async fn send_control_to_endpoint(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<domain::sync_contracts::peer_control::PeerControlSendOutcome, String> {
        match domain::sync_contracts::transport::IrohSessionTransport::send_to_transport_peer(
            self, endpoint, message,
        )
        .await?
        {
            domain::sync_contracts::transport::AddressedSendOutcome::Sent => {
                Ok(domain::sync_contracts::peer_control::PeerControlSendOutcome::Sent)
            }
            domain::sync_contracts::transport::AddressedSendOutcome::PeerOffline => {
                Ok(domain::sync_contracts::peer_control::PeerControlSendOutcome::PeerOffline)
            }
        }
    }

    fn mark_peer_data_ready(&self, peer: &str, endpoint: &str) -> Result<(), String> {
        domain::sync_contracts::transport::IrohSessionTransport::mark_peer_data_ready(
            self, peer, endpoint,
        )
    }

    fn unbind_peer(&self, peer: &str) -> Result<(), String> {
        domain::sync_contracts::transport::IrohSessionTransport::unbind_peer(self, peer)
    }

    fn disconnect_peer(
        &self,
        peer: &str,
    ) -> Result<domain::sync_contracts::transport::PeerDisconnectOutcome, String> {
        domain::sync_contracts::transport::IrohSessionTransport::disconnect_peer(self, peer)
    }

    fn disconnect_transport_endpoint(
        &self,
        endpoint: &str,
    ) -> Result<domain::sync_contracts::transport::PeerDisconnectOutcome, String> {
        domain::sync_contracts::transport::IrohSessionTransport::disconnect_transport_peer(
            self, endpoint,
        )
    }
}
