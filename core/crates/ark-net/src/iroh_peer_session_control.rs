#[async_trait::async_trait]
impl ark_sync::relay_registry::PeerSessionControl for IrohTransport {
    async fn send_control_to_endpoint(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<ark_sync::relay_registry::PeerControlSendOutcome, String> {
        match ark_sync::transport::IrohSessionTransport::send_to_transport_peer(
            self, endpoint, message,
        )
        .await?
        {
            ark_sync::transport::AddressedSendOutcome::Sent => {
                Ok(ark_sync::relay_registry::PeerControlSendOutcome::Sent)
            }
            ark_sync::transport::AddressedSendOutcome::PeerOffline => {
                Ok(ark_sync::relay_registry::PeerControlSendOutcome::PeerOffline)
            }
        }
    }

    fn mark_peer_data_ready(&self, peer: &str, endpoint: &str) -> Result<(), String> {
        ark_sync::transport::IrohSessionTransport::mark_peer_data_ready(self, peer, endpoint)
    }

    fn unbind_peer(&self, peer: &str) -> Result<(), String> {
        ark_sync::transport::IrohSessionTransport::unbind_peer(self, peer)
    }

    fn disconnect_peer(
        &self,
        peer: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        ark_sync::transport::IrohSessionTransport::disconnect_peer(self, peer)
    }

    fn disconnect_transport_endpoint(
        &self,
        endpoint: &str,
    ) -> Result<ark_sync::transport::PeerDisconnectOutcome, String> {
        ark_sync::transport::IrohSessionTransport::disconnect_transport_peer(self, endpoint)
    }
}
