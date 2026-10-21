use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationCredentialEnvelope, IntegrationNodeGrant,
    IntegrationRefreshLease,
};
use ark_domain::types::VersionVector;

pub trait IntegrationReplicationReadPort {
    fn authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String>;
    fn grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String>;
    fn configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String>;
    fn refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String>;
    fn credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String>;
    fn latest_credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String>;
    fn credential_generations_for_epoch(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        grant_epoch: u64,
    ) -> Result<Vec<u64>, String>;
    fn version_vector(&self) -> Result<Option<VersionVector>, String>;
}
