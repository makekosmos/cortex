use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::{AuthorizedNode, GrantStatus, IntegrationNodeGrant, NodeStatus, SignedSyncEnvelope};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeAuthorizationOperation {
    Authorize,
    Revoke,
    Rotate,
}

/// The host signs `envelope.canonical_signing_bytes()`; Core never receives a
/// private key or a credential plaintext.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedSyncPreparation {
    pub envelope: SignedSyncEnvelope,
    pub canonical_signing_bytes_hex: String,
}

pub fn prepare_signed_sync(
    conn: &Connection,
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

    let origin = crate::db::load_authorized_node(conn, origin_node_id)?
        .ok_or_else(|| "origin node is not authorized in this database".to_string())?;
    if origin.status != NodeStatus::Active {
        return Err("origin node is revoked".into());
    }
    require_current_grant(conn, integration_id, &origin)?;
    let recipient = crate::db::load_authorized_node(conn, recipient_node_id)?
        .ok_or_else(|| "recipient node is not authorized in this database".to_string())?;
    if recipient.status != NodeStatus::Active {
        return Err("recipient node is revoked".into());
    }
    require_current_grant(conn, integration_id, &recipient)?;

    let payload =
        crate::db::export_integration_replication(conn, integration_id, recipient_node_id)?;
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
    let canonical_signing_bytes_hex = super::encode_hex(
        &envelope
            .canonical_signing_bytes()
            .map_err(|error| error.to_string())?,
    );
    Ok(SignedSyncPreparation {
        envelope,
        canonical_signing_bytes_hex,
    })
}

pub fn persist_node_authorization(
    conn: &Connection,
    operation: NodeAuthorizationOperation,
    node: &AuthorizedNode,
    grant: Option<&IntegrationNodeGrant>,
    device_id: &str,
) -> Result<(), String> {
    with_immediate_write(conn, || {
        let current = crate::db::load_authorized_node(conn, &node.node_id)?;
        validate_node_transition(operation, current.as_ref(), node, grant)?;
        if current.as_ref() != Some(node) {
            crate::db::upsert_authorized_node(conn, node, device_id)?;
        }
        if let Some(grant) = grant {
            crate::db::upsert_integration_node_grant(conn, grant, device_id)?;
        }
        Ok(())
    })
}

pub fn persist_integration_grant(
    conn: &Connection,
    grant: &IntegrationNodeGrant,
    device_id: &str,
) -> Result<(), String> {
    with_immediate_write(conn, || {
        let node = crate::db::load_authorized_node(conn, &grant.node_id)?
            .ok_or_else(|| "integration grant node is not authorized".to_string())?;
        if grant.node_encryption_key != node.encryption_public_key
            || grant.grant_epoch < node.grant_epoch
        {
            return Err("integration grant does not match the node key epoch".into());
        }
        crate::db::upsert_integration_node_grant(conn, grant, device_id)
    })
}

pub(crate) fn validate_node_transition(
    operation: NodeAuthorizationOperation,
    current: Option<&AuthorizedNode>,
    node: &AuthorizedNode,
    grant: Option<&IntegrationNodeGrant>,
) -> Result<(), String> {
    node.validate().map_err(|error| error.to_string())?;
    match operation {
        NodeAuthorizationOperation::Authorize => {
            if current.is_some_and(|value| value != node) {
                return Err("authorize cannot replace an existing node; use rotate".into());
            }
            if node.status != NodeStatus::Active {
                return Err("authorize requires an active node record".into());
            }
        }
        NodeAuthorizationOperation::Revoke => {
            let current = current.ok_or_else(|| "node is not authorized".to_string())?;
            if current.status != NodeStatus::Active || node.status != NodeStatus::Revoked {
                return Err("revoke requires an active node and a revoked successor".into());
            }
            if node.grant_epoch <= current.grant_epoch {
                return Err("revoke epoch must increase".into());
            }
        }
        NodeAuthorizationOperation::Rotate => {
            let current = current.ok_or_else(|| "node is not authorized".to_string())?;
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
        grant.validate().map_err(|error| error.to_string())?;
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

fn with_immediate_write<T>(
    conn: &Connection,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if !conn.is_autocommit() {
        return Err("integration authorization requires an autocommit connection".into());
    }
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| error.to_string())?;
    match operation() {
        Ok(value) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(value),
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(error.to_string())
            }
        },
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub(crate) fn require_current_grant(
    conn: &Connection,
    integration_id: &str,
    node: &AuthorizedNode,
) -> Result<(), String> {
    let grant = crate::db::load_integration_node_grant(conn, integration_id, &node.node_id)?
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
