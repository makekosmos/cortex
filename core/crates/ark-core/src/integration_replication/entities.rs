use serde::{Deserialize, Serialize};

use super::{
    AuthorizedNode, IntegrationConfiguration, IntegrationContractError,
    IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "entity_type",
    content = "record",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum IntegrationReplicationEntity {
    IntegrationConfiguration(IntegrationConfiguration),
    AuthorizedNode(AuthorizedNode),
    IntegrationNodeGrant(IntegrationNodeGrant),
    IntegrationRefreshLease(IntegrationRefreshLease),
    IntegrationCredentialEnvelope(IntegrationCredentialEnvelope),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationReplicationChange {
    pub entity: IntegrationReplicationEntity,
    pub vector_hlc: String,
}

impl IntegrationReplicationChange {
    pub fn validate(&self) -> Result<(), IntegrationContractError> {
        if self.vector_hlc.trim().is_empty() {
            return Err(IntegrationContractError::EmptyField {
                field: "vector_hlc",
            });
        }
        match &self.entity {
            IntegrationReplicationEntity::IntegrationConfiguration(value) => value.validate(),
            IntegrationReplicationEntity::AuthorizedNode(value) => value.validate(),
            IntegrationReplicationEntity::IntegrationNodeGrant(value) => value.validate(),
            IntegrationReplicationEntity::IntegrationRefreshLease(value) => value.validate(),
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => value.validate(),
        }
    }

    pub fn vector_key(&self) -> String {
        match &self.entity {
            IntegrationReplicationEntity::IntegrationConfiguration(value) => {
                format!("integration_configuration:{}", value.integration_id)
            }
            IntegrationReplicationEntity::AuthorizedNode(value) => {
                format!("authorized_node:{}", value.node_id)
            }
            IntegrationReplicationEntity::IntegrationNodeGrant(value) => format!(
                "integration_node_grant:{}",
                integration_node_id(&value.integration_id, &value.node_id)
            ),
            IntegrationReplicationEntity::IntegrationRefreshLease(value) => {
                format!("integration_refresh_lease:{}", value.integration_id)
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => format!(
                "integration_credential_envelope:{}",
                envelope_entity_id(
                    &value.integration_id,
                    &value.recipient_node_id,
                    value.credential_generation,
                )
            ),
        }
    }

    pub(crate) fn dependency_rank(&self) -> u8 {
        self.entity.dependency_rank()
    }
}

impl IntegrationReplicationEntity {
    pub(crate) fn dependency_rank(&self) -> u8 {
        match self {
            IntegrationReplicationEntity::IntegrationConfiguration(_) => 0,
            IntegrationReplicationEntity::AuthorizedNode(_) => 1,
            IntegrationReplicationEntity::IntegrationNodeGrant(_) => 2,
            IntegrationReplicationEntity::IntegrationRefreshLease(_) => 3,
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(_) => 4,
        }
    }
}

pub(crate) fn integration_node_id(integration_id: &str, node_id: &str) -> String {
    format!("{}:{}{}", integration_id.len(), integration_id, node_id)
}

pub(crate) fn envelope_entity_id(
    integration_id: &str,
    recipient_node_id: &str,
    generation: u64,
) -> String {
    format!(
        "{}:{}:{}:{recipient_node_id}:{generation}",
        integration_id.len(),
        integration_id,
        recipient_node_id.len()
    )
}
