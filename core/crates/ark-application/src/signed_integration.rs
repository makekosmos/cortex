mod admission;
mod hlc;
mod persistence;

use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationConfiguration, IntegrationNodeGrant, IntegrationRefreshLease,
    IntegrationReplicationChange, IntegrationReplicationEntity, NodeStatus, SignedSyncEnvelope,
    SignedSyncError, VerifiedSyncEnvelope,
};
use ark_domain::types::VersionVector;
use std::collections::HashSet;

pub struct ApplySignedIntegrationFrame {
    pub envelope: SignedSyncEnvelope,
    pub expected_space_id: String,
    pub expected_recipient_node_id: String,
    pub authenticated_transport_public_key: Option<String>,
    pub reserved_at_ms: u64,
}

pub trait SignedIntegrationFrameStore {
    fn load_authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String>;
    fn load_integration_grant(
        &self,
        integration_id: &str,
        node_id: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String>;
    fn load_configuration(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String>;
    fn load_max_credential_generation(&self, integration_id: &str) -> Result<Option<u64>, String>;
    fn load_refresh_lease(
        &self,
        integration_id: &str,
    ) -> Result<Option<IntegrationRefreshLease>, String>;
    fn load_current_entity(
        &self,
        entity: &IntegrationReplicationEntity,
    ) -> Result<Option<IntegrationReplicationEntity>, String>;
    fn load_version_vector(&self) -> Result<VersionVector, String>;
    fn persist_entity(
        &self,
        change: &IntegrationReplicationChange,
        origin_node_id: &str,
        received_at_ms: u64,
    ) -> Result<(), String>;
    fn persist_version_vector(&self, vector: &VersionVector) -> Result<(), String>;
}

pub type ReplayApply<'a> =
    Box<dyn FnOnce(&mut dyn SignedIntegrationFrameStore) -> Result<(), SignedSyncError> + 'a>;

/// A transaction-scoped capability for applying one authenticated integration frame.
/// The callback may read and write typed records, but cannot recursively reserve replay.
pub trait SignedIntegrationFrameSession {
    fn load_authorized_node(&self, node_id: &str) -> Result<Option<AuthorizedNode>, String>;

    fn with_replay_reservation(
        &mut self,
        verified: &VerifiedSyncEnvelope,
        reserved_at_ms: u64,
        apply: ReplayApply<'_>,
    ) -> Result<(), SignedSyncError>;
}

/// Verifies and applies one addressed frame using only application-owned policy.
/// The store session keeps trust reads, replay reservation, entity writes, and vector
/// updates in the caller's existing immediate transaction.
pub fn apply_signed_integration_frame(
    session: &mut dyn SignedIntegrationFrameSession,
    request: ApplySignedIntegrationFrame,
) -> Result<(), SignedSyncError> {
    let origin = session
        .load_authorized_node(&request.envelope.origin_node_id)
        .map_err(SignedSyncError::Storage)?
        .ok_or(SignedSyncError::UnknownOrigin)?;
    if request
        .authenticated_transport_public_key
        .as_deref()
        .is_some_and(|key| origin.transport_public_key.as_deref() != Some(key))
    {
        return Err(SignedSyncError::TransportMismatch);
    }
    let recipient = session
        .load_authorized_node(&request.expected_recipient_node_id)
        .map_err(SignedSyncError::Storage)?
        .filter(|node| node.status == NodeStatus::Active)
        .ok_or(SignedSyncError::UnauthorizedRecipient)?;
    let verified = request.envelope.verify(
        &request.expected_space_id,
        &request.expected_recipient_node_id,
        &origin,
    )?;

    session.with_replay_reservation(
        &verified,
        request.reserved_at_ms,
        Box::new(|store| {
            admission::require_verified_integration_access(store, &verified, &origin, &recipient)
                .map_err(SignedSyncError::Apply)?;
            apply_verified_batch(store, &verified, request.reserved_at_ms)
        }),
    )
}

fn apply_verified_batch(
    store: &dyn SignedIntegrationFrameStore,
    verified: &VerifiedSyncEnvelope,
    now_ms: u64,
) -> Result<(), SignedSyncError> {
    let mut changes = verified.payload.clone();
    changes.sort_by_key(|change| (change.dependency_rank(), change.vector_key()));
    let mut keys = HashSet::with_capacity(changes.len());
    let mut vector = store
        .load_version_vector()
        .map_err(SignedSyncError::Storage)?;
    let mut vector_changed = false;

    for change in changes {
        change
            .validate()
            .map_err(|error| SignedSyncError::Apply(error.to_string()))?;
        hlc::require_authenticated_change_hlcs(&change, &verified.origin_node_id)
            .map_err(SignedSyncError::Apply)?;
        let key = change.vector_key();
        if !keys.insert(key.clone()) {
            return Err(SignedSyncError::Apply(format!(
                "duplicate integration vector key: {key}"
            )));
        }
        let current = store
            .load_current_entity(&change.entity)
            .map_err(SignedSyncError::Storage)?;
        if !change_requires_apply(&change, &current, &key, &vector) {
            continue;
        }
        apply_change(
            store,
            &change,
            current.as_ref(),
            &verified.origin_node_id,
            &verified.recipient_node_id,
            now_ms,
        )?;
        vector_changed |= set_integration_vector(
            &mut vector,
            key,
            change.vector_hlc,
            matches!(
                &change.entity,
                IntegrationReplicationEntity::IntegrationCredentialEnvelope(_)
            ),
        );
    }

    if vector_changed {
        store
            .persist_version_vector(&vector)
            .map_err(SignedSyncError::Storage)?;
    }
    Ok(())
}

fn apply_change(
    store: &dyn SignedIntegrationFrameStore,
    change: &IntegrationReplicationChange,
    current: Option<&IntegrationReplicationEntity>,
    origin_node_id: &str,
    recipient_node_id: &str,
    now_ms: u64,
) -> Result<(), SignedSyncError> {
    if current == Some(&change.entity) {
        return Ok(());
    }
    if let Some(integration_id) = change_integration_id(change) {
        if store
            .load_configuration(integration_id)
            .map_err(SignedSyncError::Storage)?
            .is_none()
            && !matches!(
                &change.entity,
                IntegrationReplicationEntity::IntegrationConfiguration(_)
            )
        {
            return Err(SignedSyncError::Apply(
                "integration configuration dependency is missing".into(),
            ));
        }
    }
    persistence::validate_persisted_change(
        store,
        &change.entity,
        current,
        origin_node_id,
        recipient_node_id,
        now_ms,
    )
    .map_err(SignedSyncError::Apply)?;
    store
        .persist_entity(change, origin_node_id, now_ms)
        .map_err(SignedSyncError::Storage)
}

fn change_requires_apply(
    change: &IntegrationReplicationChange,
    current: &Option<IntegrationReplicationEntity>,
    key: &str,
    vector: &VersionVector,
) -> bool {
    let IntegrationReplicationEntity::IntegrationCredentialEnvelope(remote) = &change.entity else {
        return vector_requires_change(vector, key, &change.vector_hlc);
    };
    let Some(IntegrationReplicationEntity::IntegrationCredentialEnvelope(current)) = current else {
        return true;
    };
    if current == remote {
        return vector_requires_change(vector, key, &change.vector_hlc);
    }
    envelope_version_is_newer(
        &remote.hlc,
        &remote.issuer_node_id,
        &current.hlc,
        &current.issuer_node_id,
    )
}

fn vector_requires_change(vector: &VersionVector, key: &str, remote_hlc: &str) -> bool {
    vector
        .get(key)
        .is_none_or(|local_hlc| ark_domain::hlc::HLC::is_newer(remote_hlc, local_hlc))
}

fn set_integration_vector(
    vector: &mut VersionVector,
    key: String,
    hlc: String,
    preserve_newer: bool,
) -> bool {
    if preserve_newer
        && vector
            .get(&key)
            .is_some_and(|current| !ark_domain::hlc::HLC::is_newer(&hlc, current))
    {
        return false;
    }
    vector.insert(key, hlc);
    true
}

pub(super) fn change_integration_id(change: &IntegrationReplicationChange) -> Option<&str> {
    match &change.entity {
        IntegrationReplicationEntity::IntegrationConfiguration(value) => {
            Some(&value.integration_id)
        }
        IntegrationReplicationEntity::AuthorizedNode(_) => None,
        IntegrationReplicationEntity::IntegrationNodeGrant(value) => Some(&value.integration_id),
        IntegrationReplicationEntity::IntegrationRefreshLease(value) => Some(&value.integration_id),
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
            Some(&value.integration_id)
        }
    }
}

fn envelope_version_is_newer(
    proposed_hlc: &str,
    proposed_issuer: &str,
    current_hlc: &str,
    current_issuer: &str,
) -> bool {
    ark_domain::hlc::HLC::is_newer(proposed_hlc, current_hlc)
        || (proposed_hlc == current_hlc && proposed_issuer > current_issuer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_envelope_tie_break_uses_issuer_id() {
        assert!(envelope_version_is_newer("hlc", "z-node", "hlc", "a-node"));
        assert!(!envelope_version_is_newer("hlc", "a-node", "hlc", "z-node"));
    }
}
