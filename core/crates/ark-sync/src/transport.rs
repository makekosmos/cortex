//! Storage-backed sync policy implementation for the transport contract.
use ark_application::replication_storage::IntegrationReplicationBackend;
use ark_sync_contracts::transport::OutboundIntegrationFrameAuthorizer;

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
        frame: &ark_domain::integration_replication::SignedSyncEnvelope,
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
