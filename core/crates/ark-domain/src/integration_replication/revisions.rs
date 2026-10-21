use crate::hlc::HLC;

use super::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationContractError,
    IntegrationNodeGrant, NodeStatus,
};

pub fn validate_configuration_revision(
    current: Option<&IntegrationConfiguration>,
    proposed: &IntegrationConfiguration,
) -> Result<(), IntegrationContractError> {
    proposed.validate()?;
    if let Some(current) = current {
        if current.provider != proposed.provider
            || current.account_subject != proposed.account_subject
        {
            return Err(IntegrationContractError::Mismatch {
                field: "provider_account_identity",
            });
        }
        require_newer_hlc(&proposed.hlc, &current.hlc)?;
    }
    Ok(())
}

pub fn validate_authorized_node_revision(
    current: Option<&AuthorizedNode>,
    proposed: &AuthorizedNode,
) -> Result<(), IntegrationContractError> {
    proposed.validate()?;
    if let Some(current) = current {
        if proposed.grant_epoch < current.grant_epoch {
            return Err(IntegrationContractError::NonMonotonic {
                field: "grant_epoch",
            });
        }
        if proposed.grant_epoch == current.grant_epoch {
            if current.status == NodeStatus::Revoked && proposed.status == NodeStatus::Active {
                return Err(IntegrationContractError::RevokedNode);
            }
            if current.key_fingerprint != proposed.key_fingerprint
                || current.signing_public_key != proposed.signing_public_key
                || current.encryption_public_key != proposed.encryption_public_key
                || current.transport_public_key != proposed.transport_public_key
            {
                return Err(IntegrationContractError::Mismatch {
                    field: "node_key_epoch",
                });
            }
            require_newer_hlc(&proposed.hlc, &current.hlc)?;
        }
    }
    Ok(())
}

pub fn validate_integration_grant_revision(
    current: Option<&IntegrationNodeGrant>,
    node: Option<&AuthorizedNode>,
    proposed: &IntegrationNodeGrant,
) -> Result<(), IntegrationContractError> {
    proposed.validate()?;
    if proposed.status == GrantStatus::Active {
        let node = node.ok_or(IntegrationContractError::Mismatch {
            field: "authorized_node",
        })?;
        if node.status != NodeStatus::Active {
            return Err(IntegrationContractError::RevokedNode);
        }
        if node.grant_epoch != proposed.grant_epoch
            || node.encryption_public_key != proposed.node_encryption_key
        {
            return Err(IntegrationContractError::Mismatch {
                field: "node_key_epoch",
            });
        }
    }
    if let Some(current) = current {
        if proposed.grant_epoch < current.grant_epoch {
            return Err(IntegrationContractError::NonMonotonic {
                field: "grant_epoch",
            });
        }
        if proposed.grant_epoch == current.grant_epoch {
            if current.status == GrantStatus::Revoked && proposed.status == GrantStatus::Active {
                return Err(IntegrationContractError::RevokedGrant);
            }
            if current.node_encryption_key != proposed.node_encryption_key {
                return Err(IntegrationContractError::Mismatch {
                    field: "node_key_epoch",
                });
            }
            require_newer_hlc(&proposed.hlc, &current.hlc)?;
        }
    }
    Ok(())
}

fn require_newer_hlc(proposed: &str, current: &str) -> Result<(), IntegrationContractError> {
    if HLC::is_newer(proposed, current) {
        Ok(())
    } else {
        Err(IntegrationContractError::NonMonotonic { field: "hlc" })
    }
}
