use super::integration_replication_common::{persist_with_vector, require_newer_hlc};
use crate::integration_replication::{integration_node_id, NodeStatus};
use rusqlite::Connection;

use crate::integration_replication::{
    GrantStatus as IntegrationGrantStatus, IntegrationNodeGrant as IntegrationGrant,
};

pub fn upsert_integration_node_grant(
    conn: &Connection,
    grant: &IntegrationGrant,
    device_id: &str,
) -> Result<(), String> {
    grant.validate().map_err(|e| e.to_string())?;
    let entity_id = integration_node_id(&grant.integration_id, &grant.node_id);
    persist_with_vector(
        conn,
        "integration_node_grant",
        &entity_id,
        device_id,
        || {
            if grant.status == IntegrationGrantStatus::Active {
                let node =
                    ark_store::integration_replication::load_authorized_node(conn, &grant.node_id)?
                        .ok_or_else(|| {
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
            let current = ark_store::integration_replication::load_integration_node_grant(
                conn,
                &grant.integration_id,
                &grant.node_id,
            )?;
            if let Some(current) = current {
                if grant.grant_epoch < current.grant_epoch {
                    return Err(
                        crate::integration_replication::IntegrationContractError::NonMonotonic {
                            field: "grant_epoch",
                        }
                        .to_string(),
                    );
                }
                if grant.grant_epoch == current.grant_epoch {
                    if current.status == IntegrationGrantStatus::Revoked
                        && grant.status == IntegrationGrantStatus::Active
                    {
                        return Err(
                            crate::integration_replication::IntegrationContractError::RevokedGrant
                                .to_string(),
                        );
                    }
                    if current.node_encryption_key != grant.node_encryption_key {
                        return Err(
                            crate::integration_replication::IntegrationContractError::Mismatch {
                                field: "node_key_epoch",
                            }
                            .to_string(),
                        );
                    }
                    require_newer_hlc(&grant.hlc, &current.hlc)?;
                }
            }
            ark_store::integration_replication::upsert_integration_node_grant_record(conn, grant)
        },
    )
}
