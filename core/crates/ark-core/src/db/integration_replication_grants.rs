use super::integration_replication_common::{
    persist_with_vector, require_newer_hlc, sqlite_i64, storage,
};
use super::load_authorized_node;
use crate::integration_replication::{integration_node_id, NodeStatus};
use rusqlite::{params, Connection, OptionalExtension};

use crate::integration_replication::{
    GrantStatus as IntegrationGrantStatus, IntegrationNodeGrant as IntegrationGrant,
};

fn status_value(status: IntegrationGrantStatus) -> &'static str {
    match status {
        IntegrationGrantStatus::Active => "active",
        IntegrationGrantStatus::Revoked => "revoked",
    }
}

pub fn upsert_integration_node_grant(
    conn: &Connection,
    grant: &IntegrationGrant,
    device_id: &str,
) -> Result<(), String> {
    grant.validate().map_err(|e| e.to_string())?;
    let epoch = sqlite_i64(grant.grant_epoch, "grant_epoch")?;
    let revision = sqlite_i64(grant.revision, "revision")?;
    let entity_id = integration_node_id(&grant.integration_id, &grant.node_id);
    persist_with_vector(
        conn,
        "integration_node_grant",
        &entity_id,
        device_id,
        || {
            if grant.status == IntegrationGrantStatus::Active {
                let node = load_authorized_node(conn, &grant.node_id)?.ok_or_else(|| {
                    crate::integration_replication::IntegrationContractError::Mismatch {
                        field: "authorized_node",
                    }
                    .to_string()
                })?;
                if node.status != NodeStatus::Active {
                    return Err(
                        crate::integration_replication::IntegrationContractError::RevokedNode
                            .to_string(),
                    );
                }
                if node.grant_epoch != grant.grant_epoch
                    || node.encryption_public_key != grant.node_encryption_key
                {
                    return Err(
                        crate::integration_replication::IntegrationContractError::Mismatch {
                            field: "node_key_epoch",
                        }
                        .to_string(),
                    );
                }
            }
            let current = conn
                .query_row(
                    "SELECT grant_epoch, status, node_encryption_key, hlc
                 FROM integration_node_grants
                 WHERE integration_id = ?1 AND node_id = ?2",
                    params![grant.integration_id, grant.node_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(storage)?;
            if let Some((old_epoch, old_status, encryption, hlc)) = current {
                if epoch < old_epoch {
                    return Err(
                        crate::integration_replication::IntegrationContractError::NonMonotonic {
                            field: "grant_epoch",
                        }
                        .to_string(),
                    );
                }
                if epoch == old_epoch {
                    if old_status == "revoked" && grant.status == IntegrationGrantStatus::Active {
                        return Err(
                            crate::integration_replication::IntegrationContractError::RevokedGrant
                                .to_string(),
                        );
                    }
                    if encryption != grant.node_encryption_key {
                        return Err(
                            crate::integration_replication::IntegrationContractError::Mismatch {
                                field: "node_key_epoch",
                            }
                            .to_string(),
                        );
                    }
                    require_newer_hlc(&grant.hlc, &hlc)?;
                }
            }
            conn.execute(
                "INSERT INTO integration_node_grants
             (integration_id, node_id, node_encryption_key, grant_epoch, status,
              authorized_at, revoked_at, revision, hlc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(integration_id, node_id) DO UPDATE SET
                node_encryption_key = excluded.node_encryption_key,
                grant_epoch = excluded.grant_epoch,
                status = excluded.status,
                authorized_at = excluded.authorized_at,
                revoked_at = excluded.revoked_at,
                revision = excluded.revision,
                hlc = excluded.hlc",
                params![
                    grant.integration_id,
                    grant.node_id,
                    grant.node_encryption_key,
                    epoch,
                    status_value(grant.status),
                    grant.authorized_at,
                    grant.revoked_at,
                    revision,
                    grant.hlc,
                ],
            )
            .map_err(storage)?;
            Ok(())
        },
    )
}
