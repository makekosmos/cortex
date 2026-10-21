use std::path::Path;

use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationCredentialEnvelope, IntegrationNodeGrant,
    IntegrationRefreshLease,
};

#[derive(Default)]
pub struct IntegrationReplicationReadSnapshot {
    nodes: std::collections::HashMap<String, AuthorizedNode>,
    grants: std::collections::HashMap<(String, String), IntegrationNodeGrant>,
    configurations: std::collections::HashMap<String, IntegrationConfiguration>,
    leases: std::collections::HashMap<String, IntegrationRefreshLease>,
    envelopes: std::collections::HashMap<(String, String, u64), IntegrationCredentialEnvelope>,
    version_vector: Option<ark_domain::types::VersionVector>,
}

impl IntegrationReplicationReadSnapshot {
    pub(super) fn load(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let mut connection = crate::open::open_db(path)?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let mut snapshot = Self::default();

        let node_ids = transaction
            .prepare("SELECT node_id FROM authorized_nodes")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|error| error.to_string())?;
        for node_id in node_ids {
            if let Some(node) =
                crate::integration_replication::load_authorized_node(&transaction, &node_id)?
            {
                snapshot.nodes.insert(node_id, node);
            }
        }

        let grants = transaction
            .prepare("SELECT integration_id, node_id FROM integration_node_grants")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|error| error.to_string())?;
        for (integration_id, node_id) in grants {
            if let Some(grant) = crate::integration_replication::load_integration_node_grant(
                &transaction,
                &integration_id,
                &node_id,
            )? {
                snapshot.grants.insert((integration_id, node_id), grant);
            }
        }

        let configuration_ids = transaction
            .prepare("SELECT integration_id FROM integration_configurations")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|error| error.to_string())?;
        for integration_id in configuration_ids {
            if let Some(configuration) =
                crate::integration_replication::load_integration_configuration(
                    &transaction,
                    &integration_id,
                )?
            {
                snapshot
                    .configurations
                    .insert(integration_id, configuration);
            }
        }

        let lease_ids = transaction
            .prepare("SELECT integration_id FROM integration_refresh_leases")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|error| error.to_string())?;
        for integration_id in lease_ids {
            if let Some(lease) = crate::integration_replication::load_integration_refresh_lease(
                &transaction,
                &integration_id,
            )? {
                snapshot.leases.insert(integration_id, lease);
            }
        }

        let envelopes = transaction
            .prepare(
                "SELECT integration_id, recipient_node_id, credential_generation \
                 FROM integration_credential_envelopes",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|error| error.to_string())?;
        for (integration_id, recipient_node_id, generation) in envelopes {
            let generation = u64::try_from(generation)
                .map_err(|_| "credential generation is negative".to_string())?;
            if let Some(envelope) =
                crate::integration_replication::load_integration_credential_envelope_record(
                    &transaction,
                    &integration_id,
                    &recipient_node_id,
                    generation,
                )?
            {
                snapshot
                    .envelopes
                    .insert((integration_id, recipient_node_id, generation), envelope);
            }
        }

        snapshot.version_vector =
            crate::sync_kv::get_sync_kv(&transaction, crate::VERSION_VECTOR_KEY)?
                .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
                .transpose()?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(snapshot)
    }
}

impl ark_application::integration_replication_read::IntegrationReplicationReadPort
    for IntegrationReplicationReadSnapshot
{
    fn authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String> {
        Ok(self.nodes.get(node_id).cloned())
    }

    fn grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        Ok(self
            .grants
            .get(&(integration_id.to_owned(), node_id.to_owned()))
            .cloned())
    }

    fn configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        Ok(self.configurations.get(integration_id).cloned())
    }

    fn refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String> {
        Ok(self.leases.get(integration_id).cloned())
    }

    fn credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        Ok(self
            .envelopes
            .get(&(
                integration_id.to_owned(),
                recipient_node_id.to_owned(),
                generation,
            ))
            .cloned())
    }

    fn latest_credential_envelope(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        Ok(self
            .envelopes
            .values()
            .filter(|envelope| {
                envelope.integration_id == integration_id
                    && envelope.recipient_node_id == recipient_node_id
            })
            .max_by_key(|envelope| envelope.credential_generation)
            .cloned())
    }

    fn credential_generations_for_epoch(
        &self,
        integration_id: &str,
        recipient_node_id: &str,
        grant_epoch: u64,
    ) -> Result<Vec<u64>, String> {
        let mut generations = self
            .envelopes
            .values()
            .filter(|envelope| {
                envelope.integration_id == integration_id
                    && envelope.recipient_node_id == recipient_node_id
                    && envelope.grant_epoch == grant_epoch
            })
            .map(|envelope| envelope.credential_generation)
            .collect::<Vec<_>>();
        generations.sort_unstable();
        Ok(generations)
    }

    fn version_vector(&self) -> Result<Option<ark_domain::types::VersionVector>, String> {
        Ok(self.version_vector.clone())
    }
}
