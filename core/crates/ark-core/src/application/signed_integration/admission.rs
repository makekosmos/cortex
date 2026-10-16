use super::{change_integration_id, SignedIntegrationFrameStore};
use crate::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationNodeGrant, IntegrationReplicationEntity, NodeStatus,
    VerifiedSyncEnvelope,
};
use std::collections::BTreeSet;

pub(super) fn require_verified_integration_access(
    store: &dyn SignedIntegrationFrameStore,
    verified: &VerifiedSyncEnvelope,
    origin: &AuthorizedNode,
    recipient: &AuthorizedNode,
) -> Result<(), String> {
    let integrations = verified
        .payload
        .iter()
        .filter_map(change_integration_id)
        .collect::<BTreeSet<_>>();
    if integrations.is_empty() {
        return Err("signed integration payload has no integration scope".into());
    }
    for integration_id in &integrations {
        require_existing_grant(store, integration_id, origin)?;
        require_existing_grant(store, integration_id, recipient)?;
    }
    require_scoped_node_changes(store, verified, &integrations)?;
    require_scoped_grant_changes(store, verified, &integrations)
}

fn require_existing_grant(
    store: &dyn SignedIntegrationFrameStore,
    integration_id: &str,
    node: &AuthorizedNode,
) -> Result<(), String> {
    let grant = store
        .load_integration_grant(integration_id, &node.node_id)?
        .ok_or_else(|| {
            format!(
                "node {} has no pre-existing integration grant",
                node.node_id
            )
        })?;
    require_current_grant(&grant, integration_id, node)
}

fn require_scoped_node_changes(
    store: &dyn SignedIntegrationFrameStore,
    verified: &VerifiedSyncEnvelope,
    integrations: &BTreeSet<&str>,
) -> Result<(), String> {
    for change in &verified.payload {
        let IntegrationReplicationEntity::AuthorizedNode(node) = &change.entity else {
            continue;
        };
        require_preexisting_node_trust(store, integrations, &node.node_id)?;
        if store.load_authorized_node(&node.node_id)?.as_ref() == Some(node) {
            continue;
        }
        let expected_status = match node.status {
            NodeStatus::Active => GrantStatus::Active,
            NodeStatus::Revoked => GrantStatus::Revoked,
        };
        let scoped = integrations.iter().any(|integration_id| {
            payload_grant(verified, integration_id, &node.node_id).is_some_and(|grant| {
                grant.status == expected_status
                    && grant.grant_epoch == node.grant_epoch
                    && grant.node_encryption_key == node.encryption_public_key
            })
        });
        if !scoped {
            return Err(format!(
                "authorized node {} has no matching scoped grant",
                node.node_id
            ));
        }
    }
    Ok(())
}

fn require_scoped_grant_changes(
    store: &dyn SignedIntegrationFrameStore,
    verified: &VerifiedSyncEnvelope,
    integrations: &BTreeSet<&str>,
) -> Result<(), String> {
    for change in &verified.payload {
        let IntegrationReplicationEntity::IntegrationNodeGrant(grant) = &change.entity else {
            continue;
        };
        if !integrations.contains(grant.integration_id.as_str()) {
            return Err(format!(
                "integration grant {} is outside the signed integration scope",
                grant.integration_id
            ));
        }
        require_preexisting_integration_trust(store, &grant.integration_id, &grant.node_id)?;
    }
    Ok(())
}

fn require_preexisting_node_trust(
    store: &dyn SignedIntegrationFrameStore,
    integrations: &BTreeSet<&str>,
    node_id: &str,
) -> Result<AuthorizedNode, String> {
    let node = store
        .load_authorized_node(node_id)?
        .ok_or_else(|| format!("node {node_id} has no pre-existing authorization"))?;
    for integration_id in integrations {
        if require_existing_grant(store, integration_id, &node).is_ok() {
            return Ok(node);
        }
    }
    Err(format!(
        "node {node_id} has no pre-existing active integration trust"
    ))
}

fn require_preexisting_integration_trust(
    store: &dyn SignedIntegrationFrameStore,
    integration_id: &str,
    node_id: &str,
) -> Result<AuthorizedNode, String> {
    let node = store
        .load_authorized_node(node_id)?
        .ok_or_else(|| format!("node {node_id} has no pre-existing authorization"))?;
    require_existing_grant(store, integration_id, &node)?;
    Ok(node)
}

fn payload_grant<'a>(
    verified: &'a VerifiedSyncEnvelope,
    integration_id: &str,
    node_id: &str,
) -> Option<&'a IntegrationNodeGrant> {
    verified
        .payload
        .iter()
        .find_map(|change| match &change.entity {
            IntegrationReplicationEntity::IntegrationNodeGrant(grant)
                if grant.integration_id == integration_id && grant.node_id == node_id =>
            {
                Some(grant)
            }
            _ => None,
        })
}

fn require_current_grant(
    grant: &IntegrationNodeGrant,
    integration_id: &str,
    node: &AuthorizedNode,
) -> Result<(), String> {
    grant.validate().map_err(|error| error.to_string())?;
    if grant.status != GrantStatus::Active
        || grant.integration_id != integration_id
        || grant.node_id != node.node_id
        || grant.grant_epoch != node.grant_epoch
        || grant.node_encryption_key != node.encryption_public_key
    {
        return Err(format!(
            "node {} integration grant is not current",
            node.node_id
        ));
    }
    Ok(())
}
