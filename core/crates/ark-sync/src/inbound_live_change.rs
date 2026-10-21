use ark_application::replication_storage::AuthenticatedEntityBackend;
use ark_application::sync_policy::SyncProfile;
use ark_application::sync_storage::{InboundApplyOutcome, PeerProgressPort};
use ark_domain::sync_policy::PolicyIdentity;
use ark_domain::types::SyncEntity;

use crate::protocol::{is_usage_entity, merge_usage_cursors, observe_non_usage_entity};
use ark_sync_contracts::protocol_negotiation::NegotiatedProtocol;
use ark_sync_contracts::protocol_negotiation::ProtocolCapability;

/// An inbound change that has passed policy validation and, when supported,
/// has had its durable receipt recorded. The caller must send a receipt ACK
/// before passing this value to [`apply_live_change`].
pub struct PreparedLiveChange {
    entity: SyncEntity,
    change_id: String,
    peer_device_id: String,
    local_device_id: String,
    expected_policy_identity: Option<PolicyIdentity>,
    pub receipt_recorded: bool,
}

pub struct IncomingLiveChange<'a> {
    pub protocol: Option<&'a NegotiatedProtocol>,
    pub local_profile: Option<&'a SyncProfile>,
    pub local_device_id: &'a str,
    pub peer_device_id: &'a str,
    pub change_id: String,
    pub entity: SyncEntity,
    pub policy_identity: Option<PolicyIdentity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveChangeDisposition {
    Applied { notify_local: bool },
    Stale,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionVectorWriteMode {
    /// Preserve the existing merge behavior used by the server and by
    /// policy-bound client sessions.
    MergeConcurrent,
    /// Preserve the legacy unbound client's direct replacement behavior.
    Replace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveChangeResult {
    pub disposition: LiveChangeDisposition,
}

/// Validates the negotiated policy and persists the durable receipt before the
/// caller sends its receipt acknowledgement. Applying the entity is a
/// separate step so protocol ACK ordering remains explicit at the caller.
pub async fn prepare_live_change<S>(
    storage: &S,
    incoming: IncomingLiveChange<'_>,
) -> Result<PreparedLiveChange, String>
where
    S: AuthenticatedEntityBackend + ?Sized,
{
    let IncomingLiveChange {
        protocol,
        local_profile,
        local_device_id,
        peer_device_id,
        change_id,
        entity,
        policy_identity,
    } = incoming;
    let policy_bound = protocol.is_some_and(|protocol| {
        protocol
            .capabilities
            .contains(&ProtocolCapability::PolicyBoundDeliveryV1)
    });
    let expected_policy_identity =
        policy_bound.then(|| crate::protocol::sync_policy_identity(local_profile));
    if policy_identity != expected_policy_identity {
        return Err("stale_or_unnegotiated_sync_policy".into());
    }

    let durable_receipt_ack = protocol.is_some_and(|protocol| {
        protocol
            .capabilities
            .contains(&ProtocolCapability::DurableReceiptAckV1)
    });
    let receipt_recorded = if durable_receipt_ack {
        storage
            .receive_authenticated_entity_with_policy(
                &entity,
                peer_device_id,
                &change_id,
                local_device_id,
                expected_policy_identity.clone(),
            )
            .await?
    } else {
        false
    };

    Ok(PreparedLiveChange {
        entity,
        change_id,
        peer_device_id: peer_device_id.to_owned(),
        local_device_id: local_device_id.to_owned(),
        expected_policy_identity,
        receipt_recorded,
    })
}

/// Applies a prepared change and persists the merged version vector when the
/// incoming HLC advances local progress. Receipt ACK transmission belongs
/// between `prepare_live_change` and this function.
pub async fn apply_live_change<S>(
    storage: &S,
    prepared: PreparedLiveChange,
    vector_write_mode: VersionVectorWriteMode,
) -> Result<LiveChangeResult, String>
where
    S: AuthenticatedEntityBackend + PeerProgressPort + ?Sized,
{
    let mut local_vector = storage.load_version_vector().await?;
    let should_apply = is_usage_entity(&prepared.entity)
        || local_vector
            .get(&prepared.entity.id)
            .is_none_or(|hlc| ark_domain::hlc::HLC::is_newer(&prepared.entity.hlc, hlc));
    let outcome = storage
        .apply_authenticated_entity_with_policy(
            &prepared.entity,
            &prepared.peer_device_id,
            &prepared.change_id,
            &prepared.local_device_id,
            prepared.expected_policy_identity.clone(),
        )
        .await?;

    let disposition = match outcome {
        InboundApplyOutcome::Applied => {
            if should_apply && observe_non_usage_entity(&mut local_vector, &prepared.entity) {
                let stored_vector = storage.load_version_vector().await?;
                merge_usage_cursors(&mut local_vector, &stored_vector);

                let vector_to_store = local_vector;
                match prepared.expected_policy_identity.as_ref() {
                    Some(identity) => {
                        let persist = match vector_write_mode {
                            VersionVectorWriteMode::MergeConcurrent => storage
                                .merge_version_vector_with_policy(
                                    &prepared.local_device_id,
                                    identity,
                                    &vector_to_store,
                                ),
                            VersionVectorWriteMode::Replace => storage
                                .replace_version_vector_with_policy(
                                    &prepared.local_device_id,
                                    identity,
                                    &vector_to_store,
                                ),
                        };
                        persist.await?;
                    }
                    None => match vector_write_mode {
                        VersionVectorWriteMode::MergeConcurrent => {
                            storage.merge_version_vector(&vector_to_store).await?
                        }
                        VersionVectorWriteMode::Replace => {
                            storage.replace_version_vector(&vector_to_store).await?
                        }
                    },
                }
            }
            LiveChangeDisposition::Applied {
                notify_local: should_apply,
            }
        }
        InboundApplyOutcome::Stale => LiveChangeDisposition::Stale,
        InboundApplyOutcome::Pending => LiveChangeDisposition::Pending,
    };

    Ok(LiveChangeResult { disposition })
}

#[cfg(test)]
#[path = "inbound_live_change_tests.rs"]
mod tests;
