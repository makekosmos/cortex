#[cfg(test)]
use super::integration_replication_common::{lease_entity_id, persist_with_vector};
#[cfg(test)]
use super::load_integration_node_grant;
#[cfg(test)]
use crate::integration_replication::{GrantStatus, IntegrationContractError, NodeStatus};
#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
use crate::integration_replication::IntegrationRefreshLease as ReplicatedRefreshLease;

#[cfg(test)]
pub(in crate::db) fn upsert_integration_refresh_lease_record(
    conn: &Connection,
    lease: &ReplicatedRefreshLease,
    device_id: &str,
    allow_equal_fence: bool,
) -> Result<(), String> {
    lease.validate().map_err(|e| e.to_string())?;
    let holder =
        ark_store::integration_replication::load_authorized_node(conn, &lease.holder_node_id)?
            .ok_or_else(|| {
                IntegrationContractError::Mismatch {
                    field: "authorized_node",
                }
                .to_string()
            })?;
    let grant = load_integration_node_grant(conn, &lease.integration_id, &lease.holder_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    if holder.status != NodeStatus::Active {
        return Err(IntegrationContractError::RevokedNode.to_string());
    }
    if grant.status != GrantStatus::Active {
        return Err(IntegrationContractError::RevokedGrant.to_string());
    }
    if holder.grant_epoch != grant.grant_epoch
        || holder.encryption_public_key != grant.node_encryption_key
    {
        return Err(IntegrationContractError::Mismatch {
            field: "node_key_epoch",
        }
        .to_string());
    }
    persist_with_vector(
        conn,
        "integration_refresh_lease",
        lease_entity_id(&lease.integration_id),
        device_id,
        || {
            let current = ark_store::integration_replication::load_integration_refresh_lease(
                conn,
                &lease.integration_id,
            )?;
            if let Some(current) = current {
                if lease.credential_generation < current.credential_generation {
                    return Err(IntegrationContractError::NonMonotonic {
                        field: "credential_generation",
                    }
                    .to_string());
                }
                if lease.fencing_token < current.fencing_token
                    || (!allow_equal_fence && lease.fencing_token == current.fencing_token)
                {
                    return Err(IntegrationContractError::NonMonotonic {
                        field: "fencing_token",
                    }
                    .to_string());
                }
            }
            ark_store::integration_replication::upsert_integration_refresh_lease_record(conn, lease)
        },
    )
}
