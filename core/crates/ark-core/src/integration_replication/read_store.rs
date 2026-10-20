use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationCredentialEnvelope, IntegrationNodeGrant,
    IntegrationRefreshLease,
};
use ark_domain::types::VersionVector;
#[cfg(test)]
use rusqlite::Connection;

pub trait IntegrationReplicationReadStore {
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

impl IntegrationReplicationReadStore for ark_store::IntegrationReplicationReadCapabilities<'_> {
    fn authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        ark_store::IntegrationReplicationReadCapabilities::authorized_node(self, node_id)
    }
    fn grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        ark_store::IntegrationReplicationReadCapabilities::grant(self, integration_id, node_id)
    }
    fn configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        ark_store::IntegrationReplicationReadCapabilities::configuration(self, integration_id)
    }
    fn refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String> {
        ark_store::IntegrationReplicationReadCapabilities::refresh_lease(self, integration_id)
    }
    fn credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        ark_store::IntegrationReplicationReadCapabilities::credential_envelope(
            self,
            integration_id,
            recipient_node_id,
            generation,
        )
    }
    fn latest_credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        ark_store::IntegrationReplicationReadCapabilities::latest_credential_envelope(
            self,
            integration_id,
            recipient_node_id,
        )
    }
    fn credential_generations_for_epoch(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        grant_epoch: u64,
    ) -> Result<Vec<u64>, String> {
        ark_store::IntegrationReplicationReadCapabilities::credential_generations_for_epoch(
            self,
            integration_id,
            recipient_node_id,
            grant_epoch,
        )
    }
    fn version_vector(&self) -> Result<Option<VersionVector>, String> {
        ark_store::IntegrationReplicationReadCapabilities::version_vector(self)
    }
}

#[cfg(test)]
impl IntegrationReplicationReadStore for Connection {
    fn authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        ark_store::integration_replication::load_authorized_node(self, node_id)
    }
    fn grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        ark_store::integration_replication::load_integration_node_grant(
            self,
            integration_id,
            node_id,
        )
    }
    fn configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        ark_store::integration_replication::load_integration_configuration(self, integration_id)
    }
    fn refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String> {
        ark_store::integration_replication::load_integration_refresh_lease(self, integration_id)
    }
    fn credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        ark_store::integration_replication::load_integration_credential_envelope_record(
            self,
            integration_id,
            recipient_node_id,
            generation,
        )
    }
    fn latest_credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        ark_store::integration_replication::load_latest_integration_credential_envelope_record(
            self,
            integration_id,
            recipient_node_id,
        )
    }
    fn credential_generations_for_epoch(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        grant_epoch: u64,
    ) -> Result<Vec<u64>, String> {
        ark_store::integration_replication::list_integration_credential_generations_for_epoch(
            self,
            integration_id,
            recipient_node_id,
            grant_epoch,
        )
    }
    fn version_vector(&self) -> Result<Option<VersionVector>, String> {
        ark_store::sync_kv::get_sync_kv(self, ark_store::VERSION_VECTOR_KEY)?
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }
}

#[cfg(test)]
impl IntegrationReplicationReadStore for rusqlite::Transaction<'_> {
    fn authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        <Connection as IntegrationReplicationReadStore>::authorized_node(self, node_id)
    }
    fn grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        <Connection as IntegrationReplicationReadStore>::grant(self, integration_id, node_id)
    }
    fn configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        <Connection as IntegrationReplicationReadStore>::configuration(self, integration_id)
    }
    fn refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String> {
        <Connection as IntegrationReplicationReadStore>::refresh_lease(self, integration_id)
    }
    fn credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        <Connection as IntegrationReplicationReadStore>::credential_envelope(
            self,
            integration_id,
            recipient_node_id,
            generation,
        )
    }
    fn latest_credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        <Connection as IntegrationReplicationReadStore>::latest_credential_envelope(
            self,
            integration_id,
            recipient_node_id,
        )
    }
    fn credential_generations_for_epoch(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        grant_epoch: u64,
    ) -> Result<Vec<u64>, String> {
        <Connection as IntegrationReplicationReadStore>::credential_generations_for_epoch(
            self,
            integration_id,
            recipient_node_id,
            grant_epoch,
        )
    }
    fn version_vector(&self) -> Result<Option<VersionVector>, String> {
        <Connection as IntegrationReplicationReadStore>::version_vector(self)
    }
}
