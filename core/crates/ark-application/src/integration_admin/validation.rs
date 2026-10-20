use super::{IntegrationAdminStore, NodeAuthorizationOperation};
use ark_domain::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationContractError, IntegrationNodeGrant, NodeStatus,
};

pub(super) fn validate_node_transition(
    operation: NodeAuthorizationOperation,
    current: Option<&AuthorizedNode>,
    node: &AuthorizedNode,
    grant: Option<&IntegrationNodeGrant>,
) -> Result<(), String> {
    node.validate().map_err(|e| e.to_string())?;
    match operation {
        NodeAuthorizationOperation::Authorize => {
            if current.is_some_and(|v| v != node) {
                return Err("authorize cannot replace an existing node; use rotate".into());
            }
            if node.status != NodeStatus::Active {
                return Err("authorize requires an active node record".into());
            }
        }
        NodeAuthorizationOperation::Revoke => {
            let current = current.ok_or_else(|| "node is not authorized".to_owned())?;
            if current.status != NodeStatus::Active || node.status != NodeStatus::Revoked {
                return Err("revoke requires an active node and a revoked successor".into());
            }
            if node.grant_epoch <= current.grant_epoch {
                return Err("revoke epoch must increase".into());
            }
        }
        NodeAuthorizationOperation::Rotate => {
            let current = current.ok_or_else(|| "node is not authorized".to_owned())?;
            if node.status != NodeStatus::Active || node.grant_epoch <= current.grant_epoch {
                return Err("rotate requires an active successor with a higher epoch".into());
            }
            if node.encryption_public_key == current.encryption_public_key {
                return Err("rotate requires a new encryption public key".into());
            }
            if node.key_fingerprint == current.key_fingerprint {
                return Err("rotate requires a new key fingerprint".into());
            }
        }
    }
    if let Some(grant) = grant {
        grant.validate().map_err(|e| e.to_string())?;
        if grant.node_id != node.node_id
            || grant.grant_epoch != node.grant_epoch
            || grant.node_encryption_key != node.encryption_public_key
        {
            return Err("integration grant does not match the node key epoch".into());
        }
        let expected = if operation == NodeAuthorizationOperation::Revoke {
            GrantStatus::Revoked
        } else {
            GrantStatus::Active
        };
        if grant.status != expected {
            return Err(
                "integration grant status does not match the authorization operation".into(),
            );
        }
    }
    Ok(())
}

pub(super) fn validate_grant(
    store: &dyn IntegrationAdminStore,
    grant: &IntegrationNodeGrant,
) -> Result<(), String> {
    grant.validate().map_err(|e| e.to_string())?;
    if grant.status == GrantStatus::Active {
        let node = store.load_node(&grant.node_id)?.ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "authorized_node",
            }
            .to_string()
        })?;
        require_active_node_grant(&node, grant)?;
    }
    if let Some(current) = store.load_grant(&grant.integration_id, &grant.node_id)? {
        if grant.grant_epoch < current.grant_epoch {
            return Err(IntegrationContractError::NonMonotonic {
                field: "grant_epoch",
            }
            .to_string());
        }
        if grant.grant_epoch == current.grant_epoch {
            if current.status == GrantStatus::Revoked && grant.status == GrantStatus::Active {
                return Err(IntegrationContractError::RevokedGrant.to_string());
            }
            if grant.node_encryption_key != current.node_encryption_key {
                return Err(IntegrationContractError::Mismatch {
                    field: "node_key_epoch",
                }
                .to_string());
            }
            if !ark_domain::hlc::HLC::is_newer(&grant.hlc, &current.hlc) {
                return Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string());
            }
        }
    }
    Ok(())
}

pub(super) fn require_active_node_grant(
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
