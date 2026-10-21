use std::collections::BTreeSet;

use ark_domain::integration_replication::{
    encode_hex, validate_envelope_recipient, GrantStatus, IntegrationReplicationEntity, NodeStatus,
    SignedSyncEnvelope, VerifiedSyncEnvelope,
};
use serde::{Deserialize, Serialize};

use crate::integration_replication_read::IntegrationReplicationReadPort;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedSyncPreparation {
    pub envelope: SignedSyncEnvelope,
    pub canonical_signing_bytes_hex: String,
}

pub fn prepare_signed_sync<S: IntegrationReplicationReadPort>(
    store: &S,
    space_id: &str,
    origin_node_id: &str,
    integration_id: &str,
    recipient_node_id: &str,
    message_id: &str,
) -> Result<SignedSyncPreparation, String> {
    for (value, field) in [
        (space_id, "space_id"),
        (origin_node_id, "origin_node_id"),
        (integration_id, "integration_id"),
        (recipient_node_id, "recipient_node_id"),
        (message_id, "message_id"),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{field} must not be empty"));
        }
    }

    let origin = store
        .authorized_node(origin_node_id)?
        .ok_or_else(|| "origin node is not authorized in this database".to_string())?;
    if origin.status != NodeStatus::Active {
        return Err("origin node is revoked".into());
    }
    require_current_grant(store, integration_id, &origin)?;

    let recipient = store
        .authorized_node(recipient_node_id)?
        .ok_or_else(|| "recipient node is not authorized in this database".to_string())?;
    if recipient.status != NodeStatus::Active {
        return Err("recipient node is revoked".into());
    }
    require_current_grant(store, integration_id, &recipient)?;

    let payload = crate::integration_export::export_integration_replication(
        store,
        integration_id,
        recipient_node_id,
    )?;
    if payload.is_empty() {
        return Err("integration replication export is empty".into());
    }
    let envelope = SignedSyncEnvelope::new(
        space_id,
        origin_node_id,
        recipient_node_id,
        origin.grant_epoch,
        message_id,
        payload,
        "",
    );
    let canonical_signing_bytes_hex = encode_hex(
        &envelope
            .canonical_signing_bytes()
            .map_err(|error| error.to_string())?,
    );
    Ok(SignedSyncPreparation {
        envelope,
        canonical_signing_bytes_hex,
    })
}

pub fn validate_outbound_signed_sync<S: IntegrationReplicationReadPort>(
    store: &S,
    expected_space_id: &str,
    expected_origin_node_id: &str,
    frame: &SignedSyncEnvelope,
) -> Result<(), String> {
    if frame.origin_node_id != expected_origin_node_id {
        return Err("signed integration frame origin is not this node".into());
    }
    let origin = store
        .authorized_node(expected_origin_node_id)?
        .ok_or_else(|| "origin node is not authorized in this database".to_string())?;
    let verified = frame
        .verify(expected_space_id, &frame.recipient_node_id, &origin)
        .map_err(|error| error.to_string())?;
    let recipient = store
        .authorized_node(&frame.recipient_node_id)?
        .ok_or_else(|| "recipient node is not authorized in this database".to_string())?;
    if recipient.status != NodeStatus::Active {
        return Err("recipient node is revoked".into());
    }

    let integrations = integration_ids(&verified);
    if integrations.is_empty() {
        return Err("signed integration payload has no integration scope".into());
    }
    for integration_id in integrations {
        require_current_grant(store, integration_id, &origin)?;
        require_current_grant(store, integration_id, &recipient)?;
    }
    for change in &verified.payload {
        match &change.entity {
            IntegrationReplicationEntity::IntegrationRefreshLease(lease)
                if lease.holder_node_id != verified.origin_node_id =>
            {
                return Err("refresh lease holder does not match signed origin".into());
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(envelope) => {
                if envelope.issuer_node_id != verified.origin_node_id {
                    return Err("credential envelope issuer does not match signed origin".into());
                }
                let grant = store
                    .grant(&envelope.integration_id, &recipient.node_id)?
                    .ok_or_else(|| "recipient node has no integration grant".to_string())?;
                validate_envelope_recipient(envelope, &recipient, &grant)
                    .map_err(|error| error.to_string())?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn validate_outbound_signed_sync_with_transport<S: IntegrationReplicationReadPort>(
    store: &S,
    expected_space_id: &str,
    expected_origin_node_id: &str,
    frame: &SignedSyncEnvelope,
    transport_public_key: &str,
) -> Result<(), String> {
    validate_outbound_signed_sync(store, expected_space_id, expected_origin_node_id, frame)?;
    let recipient = store
        .authorized_node(&frame.recipient_node_id)?
        .ok_or_else(|| "recipient node is not authorized in this database".to_string())?;
    if recipient.transport_public_key.as_deref() != Some(transport_public_key) {
        return Err("recipient transport identity is no longer authorized".into());
    }
    Ok(())
}

fn require_current_grant<S: IntegrationReplicationReadPort>(
    store: &S,
    integration_id: &str,
    node: &ark_domain::integration_replication::AuthorizedNode,
) -> Result<(), String> {
    let grant = store
        .grant(integration_id, &node.node_id)?
        .ok_or_else(|| format!("node {} has no integration grant", node.node_id))?;
    if grant.status != GrantStatus::Active {
        return Err(format!(
            "node {} integration grant is revoked",
            node.node_id
        ));
    }
    if grant.grant_epoch != node.grant_epoch
        || grant.node_encryption_key != node.encryption_public_key
    {
        return Err(format!("node {} integration grant is stale", node.node_id));
    }
    Ok(())
}

fn integration_ids(frame: &VerifiedSyncEnvelope) -> BTreeSet<&str> {
    frame
        .payload
        .iter()
        .filter_map(|change| match &change.entity {
            IntegrationReplicationEntity::IntegrationConfiguration(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::IntegrationNodeGrant(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::IntegrationRefreshLease(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::AuthorizedNode(_) => None,
        })
        .collect()
}
