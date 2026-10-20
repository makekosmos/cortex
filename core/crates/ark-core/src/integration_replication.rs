//! Canonical, plaintext-free contracts for replicated integrations.
//!
//! Core carries ciphertext as an opaque value and validates only its routing,
//! grant, epoch, and refresh-fencing metadata.

mod api;
mod contracts;
mod outbound;
pub(crate) mod read_store;
pub use read_store::IntegrationReplicationReadStore;

#[cfg(test)]
pub use api::{persist_integration_grant, persist_node_authorization};
pub use api::{prepare_signed_sync, NodeAuthorizationOperation, SignedSyncPreparation};
pub use ark_domain::integration_replication::encode_hex;
#[cfg(test)]
pub(crate) use ark_domain::integration_replication::envelope_entity_id;
pub use ark_domain::integration_replication::{
    acquire_refresh_lease, encryption_key_id, next_credential_generation, require_newer,
    validate_envelope_recipient, validate_plaintext_free_payload, validate_public_settings,
    validate_refresh_publication,
};
pub use ark_domain::integration_replication::{
    IntegrationReplicationChange, IntegrationReplicationEntity,
};
pub use ark_domain::integration_replication::{
    SignedSyncEnvelope, SignedSyncError, VerifiedSyncEnvelope,
};
pub use contracts::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationContractError,
    IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease,
    IntegrationVerificationStatus, IssuerEncryptionKey, IssuerEncryptionKeyForPublish, NodeStatus,
    Reauthorization,
};
#[cfg(test)]
pub use outbound::node_may_receive_integration_frames;
pub use outbound::{validate_outbound_signed_sync, validate_outbound_signed_sync_with_transport};
