//! Domain contracts and deterministic rules for signed integration replication.

mod contracts;
mod entities;
mod policy;
mod revisions;
mod signed_sync;
mod signed_sync_encoding;
#[cfg(test)]
mod tests;

pub use contracts::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationContractError,
    IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease, NodeStatus,
    Reauthorization,
};
pub use entities::{
    envelope_entity_id, integration_node_id, IntegrationReplicationChange,
    IntegrationReplicationEntity,
};
pub use policy::{
    acquire_refresh_lease, encryption_key_id, next_credential_generation, require_newer,
    validate_envelope_recipient, validate_plaintext_free_payload, validate_public_settings,
    validate_refresh_publication,
};
pub use revisions::{
    validate_authorized_node_revision, validate_configuration_revision,
    validate_integration_grant_revision,
};
pub use signed_sync::{SignedSyncEnvelope, SignedSyncError, VerifiedSyncEnvelope};
pub use signed_sync_encoding::encode_hex;
