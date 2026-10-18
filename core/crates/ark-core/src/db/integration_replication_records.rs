use super::integration_replication_common::{persist_with_vector, require_newer_hlc, storage};
use rusqlite::{params, Connection, OptionalExtension};

use crate::integration_replication::{AuthorizedNode, IntegrationConfiguration, NodeStatus};
pub fn upsert_integration_configuration(
    conn: &Connection,
    configuration: &IntegrationConfiguration,
    device_id: &str,
) -> Result<(), String> {
    configuration.validate().map_err(|e| e.to_string())?;
    persist_with_vector(
        conn,
        "integration_configuration",
        &configuration.integration_id,
        device_id,
        || {
            if let Some((provider, subject, hlc)) = conn
                .query_row(
                    "SELECT provider, account_subject, hlc FROM integration_configurations
                     WHERE integration_id = ?1",
                    params![configuration.integration_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(storage)?
            {
                if provider != configuration.provider || subject != configuration.account_subject {
                    return Err(
                        crate::integration_replication::IntegrationContractError::Mismatch {
                            field: "provider_account_identity",
                        }
                        .to_string(),
                    );
                }
                require_newer_hlc(&configuration.hlc, &hlc)?;
            }
            ark_store::integration_replication::upsert_integration_configuration_record(
                conn,
                configuration,
            )
        },
    )
}
pub fn upsert_authorized_node(
    conn: &Connection,
    node: &AuthorizedNode,
    device_id: &str,
) -> Result<(), String> {
    node.validate().map_err(|e| e.to_string())?;
    persist_with_vector(conn, "authorized_node", &node.node_id, device_id, || {
        let current =
            ark_store::integration_replication::load_authorized_node(conn, &node.node_id)?;
        if let Some(current) = current {
            if node.grant_epoch < current.grant_epoch {
                return Err(
                    crate::integration_replication::IntegrationContractError::NonMonotonic {
                        field: "grant_epoch",
                    }
                    .to_string(),
                );
            }
            if node.grant_epoch == current.grant_epoch {
                if current.status == NodeStatus::Revoked && node.status == NodeStatus::Active {
                    return Err(
                        crate::integration_replication::IntegrationContractError::RevokedNode
                            .to_string(),
                    );
                }
                if current.key_fingerprint != node.key_fingerprint
                    || current.signing_public_key != node.signing_public_key
                    || current.encryption_public_key != node.encryption_public_key
                    || current.transport_public_key != node.transport_public_key
                {
                    return Err(
                        crate::integration_replication::IntegrationContractError::Mismatch {
                            field: "node_key_epoch",
                        }
                        .to_string(),
                    );
                }
                require_newer_hlc(&node.hlc, &current.hlc)?;
            }
        }
        ark_store::integration_replication::upsert_authorized_node_record(conn, node)
    })
}
