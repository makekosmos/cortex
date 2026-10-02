use std::collections::BTreeSet;

use crate::integration_replication::{
    AuthorizedNode as AdmissionNode, GrantStatus as AdmissionGrantStatus,
    IntegrationNodeGrant as AdmissionGrant, IntegrationReplicationEntity as AdmissionEntity,
    IntegrationVerificationStatus, NodeStatus as AdmissionNodeStatus,
    VerifiedSyncEnvelope as AdmissionEnvelope,
};

pub fn integration_verification_status(
    conn: &Connection,
    integration_id: &str,
    local_node_id: &str,
    now_ms: u64,
) -> Result<IntegrationVerificationStatus, String> {
    if integration_id.trim().is_empty() || local_node_id.trim().is_empty() {
        return Err("integration and local node ids must not be empty".into());
    }
    let configuration = load_integration_configuration(conn, integration_id)?;
    let node = load_authorized_node(conn, local_node_id)?;
    let grant = load_integration_node_grant(conn, integration_id, local_node_id)?;
    let grant_key_matches_node = node
        .as_ref()
        .zip(grant.as_ref())
        .is_some_and(|(node, grant)| {
            grant.grant_epoch == node.grant_epoch
                && grant.node_encryption_key == node.encryption_public_key
        });
    let access_is_current = node.as_ref().is_some_and(|node| {
        node.status == AdmissionNodeStatus::Active
            && grant.as_ref().is_some_and(|grant| {
                grant.status == AdmissionGrantStatus::Active && grant_key_matches_node
            })
    });
    let envelope = if access_is_current {
        load_latest_integration_credential_envelope(conn, integration_id, local_node_id)?
    } else {
        None
    };
    let refresh_lease = load_integration_refresh_lease(conn, integration_id)?;
    let next_generation = envelope
        .as_ref()
        .and_then(|envelope| envelope.credential_generation.checked_add(1));
    let lease_is_current = refresh_lease.as_ref().is_some_and(|lease| {
        lease.holder_node_id == local_node_id
            && lease.issued_at_ms <= now_ms
            && now_ms < lease.expires_at_ms
            && Some(lease.credential_generation) == next_generation
    });
    let lease_allows_refresh = refresh_lease
        .as_ref()
        .is_none_or(|lease| lease.expires_at_ms <= now_ms || lease_is_current);
    let ready_for_collection = configuration
        .as_ref()
        .is_some_and(|configuration| configuration.enabled)
        && access_is_current
        && envelope.is_some();

    Ok(IntegrationVerificationStatus {
        integration_id: integration_id.into(),
        local_node_id: local_node_id.into(),
        integration_present: configuration.is_some(),
        integration_enabled: configuration.as_ref().map(|value| value.enabled),
        node_status: node.as_ref().map(|value| value.status),
        node_grant_epoch: node.as_ref().map(|value| value.grant_epoch),
        grant_status: grant.as_ref().map(|value| value.status),
        grant_epoch: grant.as_ref().map(|value| value.grant_epoch),
        grant_key_matches_node,
        envelope_generation: envelope.as_ref().map(|value| value.credential_generation),
        envelope_grant_epoch: envelope.as_ref().map(|value| value.grant_epoch),
        envelope_key_id: envelope.as_ref().map(|value| value.key_id.clone()),
        envelope_available: envelope.is_some(),
        refresh_lease,
        lease_is_current,
        ready_for_collection,
        ready_for_refresh: ready_for_collection && lease_allows_refresh,
    })
}

fn require_verified_integration_access(
    conn: &Connection,
    verified: &AdmissionEnvelope,
    origin: &AdmissionNode,
    recipient: &AdmissionNode,
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
        require_existing_grant(conn, integration_id, origin)?;
        require_existing_grant(conn, integration_id, recipient)?;
    }
    require_scoped_node_changes(conn, verified, &integrations)?;
    require_scoped_grant_changes(conn, verified, &integrations)
}

fn require_existing_grant(
    conn: &Connection,
    integration_id: &str,
    node: &AdmissionNode,
) -> Result<(), String> {
    let grant =
        load_integration_node_grant(conn, integration_id, &node.node_id)?.ok_or_else(|| {
            format!(
                "node {} has no pre-existing integration grant",
                node.node_id
            )
        })?;
    require_current_grant(&grant, integration_id, node)
}

fn require_scoped_node_changes(
    conn: &Connection,
    verified: &AdmissionEnvelope,
    integrations: &BTreeSet<&str>,
) -> Result<(), String> {
    for change in &verified.payload {
        let AdmissionEntity::AuthorizedNode(node) = &change.entity else {
            continue;
        };
        require_preexisting_node_trust(conn, integrations, &node.node_id)?;
        if load_authorized_node(conn, &node.node_id)?.as_ref() == Some(node) {
            continue;
        }
        let expected_status = match node.status {
            AdmissionNodeStatus::Active => AdmissionGrantStatus::Active,
            AdmissionNodeStatus::Revoked => AdmissionGrantStatus::Revoked,
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
    conn: &Connection,
    verified: &AdmissionEnvelope,
    integrations: &BTreeSet<&str>,
) -> Result<(), String> {
    for change in &verified.payload {
        let AdmissionEntity::IntegrationNodeGrant(grant) = &change.entity else {
            continue;
        };
        if !integrations.contains(grant.integration_id.as_str()) {
            return Err(format!(
                "integration grant {} is outside the signed integration scope",
                grant.integration_id
            ));
        }
        require_preexisting_integration_trust(conn, &grant.integration_id, &grant.node_id)?;
    }
    Ok(())
}

fn require_preexisting_node_trust(
    conn: &Connection,
    integrations: &BTreeSet<&str>,
    node_id: &str,
) -> Result<AdmissionNode, String> {
    let node = load_authorized_node(conn, node_id)?
        .ok_or_else(|| format!("node {node_id} has no pre-existing authorization"))?;
    for integration_id in integrations {
        if require_existing_grant(conn, integration_id, &node).is_ok() {
            return Ok(node);
        }
    }
    Err(format!(
        "node {node_id} has no pre-existing active integration trust"
    ))
}

fn require_preexisting_integration_trust(
    conn: &Connection,
    integration_id: &str,
    node_id: &str,
) -> Result<AdmissionNode, String> {
    let node = load_authorized_node(conn, node_id)?
        .ok_or_else(|| format!("node {node_id} has no pre-existing authorization"))?;
    require_existing_grant(conn, integration_id, &node)?;
    Ok(node)
}

fn payload_grant<'a>(
    verified: &'a AdmissionEnvelope,
    integration_id: &str,
    node_id: &str,
) -> Option<&'a AdmissionGrant> {
    verified
        .payload
        .iter()
        .find_map(|change| match &change.entity {
            AdmissionEntity::IntegrationNodeGrant(grant)
                if grant.integration_id == integration_id && grant.node_id == node_id =>
            {
                Some(grant)
            }
            _ => None,
        })
}
fn require_current_grant(
    grant: &AdmissionGrant,
    integration_id: &str,
    node: &AdmissionNode,
) -> Result<(), String> {
    grant.validate().map_err(|error| error.to_string())?;
    if grant.status != AdmissionGrantStatus::Active
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
