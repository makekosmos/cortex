#[cfg(test)]
use rusqlite::Connection;
use serde::Serialize;

#[cfg(test)]
use super::IntegrationNodeGrant;
#[cfg(test)]
use super::{AuthorizedNode, GrantStatus, NodeStatus};
pub use ark_application::integration_sync::{prepare_signed_sync, SignedSyncPreparation};
#[cfg(test)]
use ark_store::integration_replication::{upsert_authorized_node, upsert_integration_node_grant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NodeAuthorizationOperation {
    Authorize,
    Revoke,
    Rotate,
}

#[cfg(test)]
pub fn persist_node_authorization(
    conn: &Connection,
    operation: NodeAuthorizationOperation,
    node: &AuthorizedNode,
    grant: Option<&IntegrationNodeGrant>,
    device_id: &str,
) -> Result<(), String> {
    with_immediate_write(conn, || {
        persist_node_authorization_inner(conn, operation, node, grant, device_id)
    })
}

#[cfg(test)]
fn persist_node_authorization_inner(
    conn: &Connection,
    operation: NodeAuthorizationOperation,
    node: &AuthorizedNode,
    grant: Option<&IntegrationNodeGrant>,
    device_id: &str,
) -> Result<(), String> {
    let current = ark_store::integration_replication::load_authorized_node(conn, &node.node_id)?;
    validate_node_transition(operation, current.as_ref(), node, grant)?;
    if current.as_ref() != Some(node) {
        upsert_authorized_node(conn, node, device_id)?;
    }
    if let Some(grant) = grant {
        upsert_integration_node_grant(conn, grant, device_id)?;
    }
    Ok(())
}

#[cfg(test)]
pub fn persist_integration_grant(
    conn: &Connection,
    grant: &IntegrationNodeGrant,
    device_id: &str,
) -> Result<(), String> {
    with_immediate_write(conn, || {
        persist_integration_grant_inner(conn, grant, device_id)
    })
}

#[cfg(test)]
fn persist_integration_grant_inner(
    conn: &Connection,
    grant: &IntegrationNodeGrant,
    device_id: &str,
) -> Result<(), String> {
    let node = ark_store::integration_replication::load_authorized_node(conn, &grant.node_id)?
        .ok_or_else(|| "integration grant node is not authorized".to_string())?;
    if grant.node_encryption_key != node.encryption_public_key
        || grant.grant_epoch < node.grant_epoch
    {
        return Err("integration grant does not match the node key epoch".into());
    }
    upsert_integration_node_grant(conn, grant, device_id)
}

#[cfg(test)]
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

#[cfg(test)]
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
