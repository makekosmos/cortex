//! Canonical, plaintext-free contracts for replicated integrations.
//!
//! Core carries ciphertext as an opaque value and validates only its routing,
//! grant, epoch, and refresh-fencing metadata.

mod api;
mod contracts;
mod outbound;

pub use api::{
    persist_integration_grant, persist_node_authorization, prepare_signed_sync,
    NodeAuthorizationOperation, SignedSyncPreparation,
};
pub(crate) use api::{
    persist_integration_grant_in_transaction, persist_node_authorization_in_transaction,
};
pub use ark_domain::integration_replication::encode_hex;
pub use ark_domain::integration_replication::{
    acquire_refresh_lease, encryption_key_id, next_credential_generation, require_newer,
    validate_envelope_recipient, validate_plaintext_free_payload, validate_public_settings,
    validate_refresh_publication,
};
pub(crate) use ark_domain::integration_replication::{envelope_entity_id, integration_node_id};
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
pub use outbound::{
    node_may_receive_integration_frames, validate_outbound_signed_sync,
    validate_outbound_signed_sync_with_transport,
};
