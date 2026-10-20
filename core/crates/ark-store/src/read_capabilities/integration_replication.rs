use super::IntegrationReplicationReadCapabilities;
use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationCredentialEnvelope, IntegrationNodeGrant,
    IntegrationRefreshLease,
};
use ark_domain::types::VersionVector;

impl IntegrationReplicationReadCapabilities<'_> {
    pub fn authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        crate::integration_replication::load_authorized_node(self.connection, node_id)
    }

    pub fn grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        crate::integration_replication::load_integration_node_grant(
            self.connection,
            integration_id,
            node_id,
        )
    }

    pub fn configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        crate::integration_replication::load_integration_configuration(
            self.connection,
            integration_id,
        )
    }

    pub fn refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String> {
        crate::integration_replication::load_integration_refresh_lease(
            self.connection,
            integration_id,
        )
    }

    pub fn credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        crate::integration_replication::load_integration_credential_envelope_record(
            self.connection,
            integration_id,
            recipient_node_id,
            generation,
        )
    }

    pub fn latest_credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        crate::integration_replication::load_latest_integration_credential_envelope_record(
            self.connection,
            integration_id,
            recipient_node_id,
        )
    }

    pub fn credential_generations_for_epoch(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        grant_epoch: u64,
    ) -> Result<Vec<u64>, String> {
        crate::integration_replication::list_integration_credential_generations_for_epoch(
            self.connection,
            integration_id,
            recipient_node_id,
            grant_epoch,
        )
    }

    pub fn version_vector(&self) -> Result<Option<VersionVector>, String> {
        crate::sync_kv::get_sync_kv(self.connection, crate::VERSION_VECTOR_KEY)?
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }
}
