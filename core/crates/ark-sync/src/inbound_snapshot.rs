use ark_domain::sync_policy::PolicyIdentity;
use std::collections::HashMap;

use ark_application::replication_storage::AuthenticatedEntityBackend;
use ark_application::sync_policy::SyncProfile;
use ark_application::sync_storage::{InboundApplyOutcome, PeerProgressPort};
use ark_domain::types::{SyncEntity, VersionVector};

use crate::protocol::{is_usage_entity, observe_non_usage_entity, sync_policy_identity};
use ark_sync_contracts::protocol_negotiation::{NegotiatedProtocol, ProtocolCapability};

pub struct InboundSnapshotBatch {
    local_device_id: String,
    peer_device_id: String,
    policy_identity: Option<PolicyIdentity>,
    local_vector: VersionVector,
    vector_updated: bool,
    accepted: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotEntityResult {
    Accepted,
    Skipped,
    Pending,
}

impl InboundSnapshotBatch {
    pub async fn begin<S>(
        storage: &S,
        local_device_id: impl Into<String>,
        peer_device_id: impl Into<String>,
        protocol: Option<&NegotiatedProtocol>,
        local_profile: Option<&SyncProfile>,
        policy_identity: Option<PolicyIdentity>,
    ) -> Result<Self, String>
    where
        S: PeerProgressPort + ?Sized,
    {
        let policy_bound = protocol.is_some_and(|protocol| {
            protocol
                .capabilities
                .contains(&ProtocolCapability::PolicyBoundDeliveryV1)
        });
        let expected_identity = policy_bound.then(|| sync_policy_identity(local_profile));
        if policy_identity != expected_identity {
            return Err("stale_or_unnegotiated_sync_policy".into());
        }

        Ok(Self {
            local_device_id: local_device_id.into(),
            peer_device_id: peer_device_id.into(),
            policy_identity,
            local_vector: storage.load_version_vector().await?,
            vector_updated: false,
            accepted: 0,
        })
    }

    pub async fn apply_entity<S>(
        &mut self,
        storage: &S,
        entity: &SyncEntity,
        message_id: &str,
    ) -> Result<SnapshotEntityResult, String>
    where
        S: AuthenticatedEntityBackend + ?Sized,
    {
        let should_apply = is_usage_entity(entity)
            || self
                .local_vector
                .get(&entity.id)
                .is_none_or(|hlc| ark_domain::hlc::HLC::is_newer(&entity.hlc, hlc));
        if !should_apply {
            return Ok(SnapshotEntityResult::Skipped);
        }

        match storage
            .apply_authenticated_entity_with_policy(
                entity,
                &self.peer_device_id,
                message_id,
                &self.local_device_id,
                self.policy_identity.clone(),
            )
            .await?
        {
            InboundApplyOutcome::Applied | InboundApplyOutcome::Stale => {
                self.vector_updated |= observe_non_usage_entity(&mut self.local_vector, entity);
                self.accepted += 1;
                Ok(SnapshotEntityResult::Accepted)
            }
            InboundApplyOutcome::Pending => Ok(SnapshotEntityResult::Pending),
        }
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted
    }

    pub async fn finish<S>(
        &mut self,
        storage: &S,
        is_last: bool,
        usage_complete_through: Option<&HashMap<String, u64>>,
    ) -> Result<(), String>
    where
        S: PeerProgressPort + ?Sized,
    {
        match self.policy_identity.as_ref() {
            Some(identity) => {
                persist_pull_vector_with_policy(
                    storage,
                    &self.local_device_id,
                    identity,
                    &mut self.local_vector,
                    self.vector_updated,
                    is_last,
                    usage_complete_through,
                )
                .await
            }
            None => {
                persist_pull_vector(
                    storage,
                    &mut self.local_vector,
                    self.vector_updated,
                    is_last,
                    usage_complete_through,
                )
                .await
            }
        }
    }
}

pub async fn persist_pull_vector<S>(
    storage: &S,
    vector: &mut VersionVector,
    vector_updated: bool,
    is_last: bool,
    usage_complete_through: Option<&HashMap<String, u64>>,
) -> Result<(), String>
where
    S: PeerProgressPort + ?Sized,
{
    if !(vector_updated || (is_last && usage_complete_through.is_some())) {
        return Ok(());
    }
    if is_last {
        if let Some(complete_through) = usage_complete_through {
            crate::protocol::apply_usage_complete_through(vector, complete_through);
        }
    }
    storage.merge_version_vector(vector).await
}

pub async fn persist_pull_vector_with_policy<S>(
    storage: &S,
    local_device_id: &str,
    policy_identity: &PolicyIdentity,
    vector: &mut VersionVector,
    vector_updated: bool,
    is_last: bool,
    usage_complete_through: Option<&HashMap<String, u64>>,
) -> Result<(), String>
where
    S: PeerProgressPort + ?Sized,
{
    if !(vector_updated || (is_last && usage_complete_through.is_some())) {
        return Ok(());
    }
    if is_last {
        if let Some(complete_through) = usage_complete_through {
            crate::protocol::apply_usage_complete_through(vector, complete_through);
        }
    }
    storage
        .merge_version_vector_with_policy(local_device_id, policy_identity, vector)
        .await
}
