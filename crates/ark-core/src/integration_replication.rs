//! Canonical, plaintext-free contracts for replicated integrations.
//!
//! Core carries ciphertext as an opaque value and validates only its routing,
//! grant, epoch, and refresh-fencing metadata.

mod api;
mod contracts;
mod entities;
mod outbound;
mod policy;
mod signed_sync;
mod signed_sync_encoding;
#[cfg(test)]
mod signed_sync_tests;
#[cfg(test)]
mod tests;

pub use api::{
    persist_integration_grant, persist_node_authorization, prepare_signed_sync,
    NodeAuthorizationOperation, SignedSyncPreparation,
};
pub use contracts::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationContractError,
    IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease,
    IntegrationVerificationStatus, NodeStatus,
};
pub(crate) use entities::{envelope_entity_id, integration_node_id};
pub use entities::{IntegrationReplicationChange, IntegrationReplicationEntity};
pub use outbound::{
    node_may_receive_integration_frames, validate_outbound_signed_sync,
    validate_outbound_signed_sync_with_transport,
};
pub use policy::{
    acquire_refresh_lease, encryption_key_id, next_credential_generation, require_newer,
    validate_envelope_recipient, validate_plaintext_free_payload, validate_public_settings,
    validate_refresh_publication,
};
pub use signed_sync::{
    verify_reserve_and_apply_signed_sync, SignedSyncEnvelope, SignedSyncError, VerifiedSyncEnvelope,
};
pub use signed_sync_encoding::encode_hex;
