use super::SignedIntegrationFrameStore;
use ark_domain::integration_replication::{
    validate_envelope_recipient, AuthorizedNode, GrantStatus, IntegrationContractError,
    IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease,
    IntegrationReplicationEntity, NodeStatus,
};

pub(super) fn validate_persisted_change(
    store: &dyn SignedIntegrationFrameStore,
    proposed: &IntegrationReplicationEntity,
    current: Option<&IntegrationReplicationEntity>,
    origin_node_id: &str,
    recipient_node_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    match proposed {
        IntegrationReplicationEntity::IntegrationConfiguration(proposed) => {
            proposed.validate().map_err(|error| error.to_string())?;
            if let Some(IntegrationReplicationEntity::IntegrationConfiguration(current)) = current {
                if proposed.provider != current.provider
                    || proposed.account_subject != current.account_subject
                {
                    return Err(IntegrationContractError::Mismatch {
                        field: "provider_account_identity",
                    }
                    .to_string());
                }
                require_newer_hlc(&proposed.hlc, &current.hlc)?;
            }
        }
        IntegrationReplicationEntity::AuthorizedNode(proposed) => {
            proposed.validate().map_err(|error| error.to_string())?;
            if let Some(IntegrationReplicationEntity::AuthorizedNode(current)) = current {
                if proposed.grant_epoch < current.grant_epoch {
                    return Err(IntegrationContractError::NonMonotonic {
                        field: "grant_epoch",
                    }
                    .to_string());
                }
                if proposed.grant_epoch == current.grant_epoch {
                    if current.status == NodeStatus::Revoked
                        && proposed.status == NodeStatus::Active
                    {
                        return Err(IntegrationContractError::RevokedNode.to_string());
                    }
                    if proposed.key_fingerprint != current.key_fingerprint
                        || proposed.signing_public_key != current.signing_public_key
                        || proposed.encryption_public_key != current.encryption_public_key
                        || proposed.transport_public_key != current.transport_public_key
                    {
                        return Err(IntegrationContractError::Mismatch {
                            field: "node_key_epoch",
                        }
                        .to_string());
                    }
                    require_newer_hlc(&proposed.hlc, &current.hlc)?;
                }
            }
        }
        IntegrationReplicationEntity::IntegrationNodeGrant(proposed) => {
            proposed.validate().map_err(|error| error.to_string())?;
            if proposed.status == GrantStatus::Active {
                let node = store
                    .load_authorized_node(&proposed.node_id)?
                    .ok_or_else(|| {
                        IntegrationContractError::Mismatch {
                            field: "authorized_node",
                        }
                        .to_string()
                    })?;
                if node.status != NodeStatus::Active {
                    return Err(IntegrationContractError::RevokedNode.to_string());
                }
                if node.grant_epoch != proposed.grant_epoch
                    || node.encryption_public_key != proposed.node_encryption_key
                {
                    return Err(IntegrationContractError::Mismatch {
                        field: "node_key_epoch",
                    }
                    .to_string());
                }
            }
            if let Some(IntegrationReplicationEntity::IntegrationNodeGrant(current)) = current {
                if proposed.grant_epoch < current.grant_epoch {
                    return Err(IntegrationContractError::NonMonotonic {
                        field: "grant_epoch",
                    }
                    .to_string());
                }
                if proposed.grant_epoch == current.grant_epoch {
                    if current.status == GrantStatus::Revoked
                        && proposed.status == GrantStatus::Active
                    {
                        return Err(IntegrationContractError::RevokedGrant.to_string());
                    }
                    if proposed.node_encryption_key != current.node_encryption_key {
                        return Err(IntegrationContractError::Mismatch {
                            field: "node_key_epoch",
                        }
                        .to_string());
                    }
                    require_newer_hlc(&proposed.hlc, &current.hlc)?;
                }
            }
        }
        IntegrationReplicationEntity::IntegrationRefreshLease(proposed) => {
            validate_refresh_lease(store, proposed, current)?;
            if proposed.holder_node_id != origin_node_id {
                return Err("refresh lease holder does not match signed origin".into());
            }
        }
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(proposed) => {
            validate_credential_envelope(store, proposed, current, recipient_node_id, now_ms)?;
            if proposed.issuer_node_id != origin_node_id {
                return Err("credential envelope issuer does not match signed origin".into());
            }
        }
    }
    Ok(())
}

fn validate_refresh_lease(
    store: &dyn SignedIntegrationFrameStore,
    proposed: &IntegrationRefreshLease,
    current: Option<&IntegrationReplicationEntity>,
) -> Result<(), String> {
    proposed.validate().map_err(|error| error.to_string())?;
    let holder = store
        .load_authorized_node(&proposed.holder_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "authorized_node",
            }
            .to_string()
        })?;
    let grant = store
        .load_integration_grant(&proposed.integration_id, &proposed.holder_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    require_active_node_grant(&holder, &grant)?;
    if let Some(IntegrationReplicationEntity::IntegrationRefreshLease(current)) = current {
        if proposed.credential_generation < current.credential_generation {
            return Err(IntegrationContractError::NonMonotonic {
                field: "credential_generation",
            }
            .to_string());
        }
        if proposed.fencing_token < current.fencing_token {
            return Err(IntegrationContractError::NonMonotonic {
                field: "fencing_token",
            }
            .to_string());
        }
    }
    Ok(())
}

fn validate_credential_envelope(
    store: &dyn SignedIntegrationFrameStore,
    proposed: &IntegrationCredentialEnvelope,
    current: Option<&IntegrationReplicationEntity>,
    recipient_node_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    proposed.validate().map_err(|error| error.to_string())?;
    if proposed.recipient_node_id != recipient_node_id {
        return Err("credential envelope recipient does not match signed frame".into());
    }
    let recipient = store
        .load_authorized_node(&proposed.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "authorized_node",
            }
            .to_string()
        })?;
    let recipient_grant = store
        .load_integration_grant(&proposed.integration_id, &proposed.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    validate_envelope_recipient(proposed, &recipient, &recipient_grant)
        .map_err(|error| error.to_string())?;
    let issuer = store
        .load_authorized_node(&proposed.issuer_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "issuer_authorized_node",
            }
            .to_string()
        })?;
    let issuer_grant = store
        .load_integration_grant(&proposed.integration_id, &proposed.issuer_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "issuer_integration_node_grant",
            }
            .to_string()
        })?;
    require_active_node_grant(&issuer, &issuer_grant)?;
    let lease = store
        .load_refresh_lease(&proposed.integration_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "refresh_lease",
            }
            .to_string()
        })?;
    if lease.is_expired(now_ms) {
        return Err(IntegrationContractError::ExpiredLease.to_string());
    }
    if lease.holder_node_id != proposed.issuer_node_id {
        return Err(IntegrationContractError::LeaseHolderMismatch.to_string());
    }
    if lease.credential_generation != proposed.credential_generation {
        return Err(IntegrationContractError::LeaseGenerationMismatch.to_string());
    }
    if lease.fencing_token != proposed.refresh_fencing_token {
        return Err(IntegrationContractError::LeaseFenceMismatch.to_string());
    }
    if store
        .load_max_credential_generation(&proposed.integration_id)?
        .is_some_and(|generation| proposed.credential_generation < generation)
    {
        return Err(IntegrationContractError::NonMonotonic {
            field: "credential_generation",
        }
        .to_string());
    }
    if let Some(IntegrationReplicationEntity::IntegrationCredentialEnvelope(current)) = current {
        if !credential_envelope_is_newer(proposed, current) {
            return Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string());
        }
    }
    Ok(())
}

fn require_active_node_grant(
    node: &AuthorizedNode,
    grant: &IntegrationNodeGrant,
) -> Result<(), String> {
    if node.status != NodeStatus::Active {
        return Err(IntegrationContractError::RevokedNode.to_string());
    }
    if grant.status != GrantStatus::Active {
        return Err(IntegrationContractError::RevokedGrant.to_string());
    }
    if node.grant_epoch != grant.grant_epoch
        || node.encryption_public_key != grant.node_encryption_key
    {
        return Err(IntegrationContractError::Mismatch {
            field: "node_key_epoch",
        }
        .to_string());
    }
    Ok(())
}

fn require_newer_hlc(proposed: &str, current: &str) -> Result<(), String> {
    if ark_domain::hlc::HLC::is_newer(proposed, current) {
        Ok(())
    } else {
        Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string())
    }
}

fn credential_envelope_is_newer(
    proposed: &IntegrationCredentialEnvelope,
    current: &IntegrationCredentialEnvelope,
) -> bool {
    ark_domain::hlc::HLC::is_newer(&proposed.hlc, &current.hlc)
        || (proposed.hlc == current.hlc && proposed.issuer_node_id > current.issuer_node_id)
}
