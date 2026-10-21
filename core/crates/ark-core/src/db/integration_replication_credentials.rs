#[cfg(test)]
use super::integration_replication_common::{envelope_version_is_newer, persist_with_vector};
#[cfg(test)]
use super::load_integration_node_grant;
use crate::integration_replication::read_store::IntegrationReplicationReadPort;
#[cfg(test)]
use crate::integration_replication::{envelope_entity_id, GrantStatus, NodeStatus};
use crate::integration_replication::{
    validate_envelope_recipient, IntegrationContractError, IntegrationCredentialEnvelope,
};
#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
pub(in crate::db) fn upsert_integration_credential_envelope_record(
    conn: &Connection,
    envelope: &IntegrationCredentialEnvelope,
    device_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    envelope.validate().map_err(|e| e.to_string())?;
    let envelope_id = envelope_entity_id(
        &envelope.integration_id,
        &envelope.recipient_node_id,
        envelope.credential_generation,
    );
    persist_with_vector(
        conn,
        "integration_credential_envelope",
        &envelope_id,
        device_id,
        || {
            let node = ark_store::integration_replication::load_authorized_node(
                conn,
                &envelope.recipient_node_id,
            )?
            .ok_or_else(|| {
                IntegrationContractError::Mismatch {
                    field: "authorized_node",
                }
                .to_string()
            })?;
            let grant = load_integration_node_grant(
                conn,
                &envelope.integration_id,
                &envelope.recipient_node_id,
            )?
            .ok_or_else(|| {
                IntegrationContractError::Mismatch {
                    field: "integration_node_grant",
                }
                .to_string()
            })?;
            validate_envelope_recipient(envelope, &node, &grant)
                .map_err(|error| error.to_string())?;
            let issuer = ark_store::integration_replication::load_authorized_node(
                conn,
                &envelope.issuer_node_id,
            )?
            .ok_or_else(|| {
                IntegrationContractError::Mismatch {
                    field: "issuer_authorized_node",
                }
                .to_string()
            })?;
            let issuer_grant = load_integration_node_grant(
                conn,
                &envelope.integration_id,
                &envelope.issuer_node_id,
            )?
            .ok_or_else(|| {
                IntegrationContractError::Mismatch {
                    field: "issuer_integration_node_grant",
                }
                .to_string()
            })?;
            if issuer.status != NodeStatus::Active {
                return Err(IntegrationContractError::RevokedNode.to_string());
            }
            if issuer_grant.status != GrantStatus::Active {
                return Err(IntegrationContractError::RevokedGrant.to_string());
            }
            if issuer.grant_epoch != issuer_grant.grant_epoch
                || issuer.encryption_public_key != issuer_grant.node_encryption_key
            {
                return Err(IntegrationContractError::Mismatch {
                    field: "issuer_node_key_epoch",
                }
                .to_string());
            }
            let lease = ark_store::integration_replication::load_integration_refresh_lease(
                conn,
                &envelope.integration_id,
            )?
            .ok_or_else(|| {
                IntegrationContractError::Mismatch {
                    field: "refresh_lease",
                }
                .to_string()
            })?;
            if lease.is_expired(now_ms) {
                return Err(IntegrationContractError::ExpiredLease.to_string());
            }
            if lease.holder_node_id != envelope.issuer_node_id {
                return Err(IntegrationContractError::LeaseHolderMismatch.to_string());
            }
            if lease.credential_generation != envelope.credential_generation {
                return Err(IntegrationContractError::LeaseGenerationMismatch.to_string());
            }
            if lease.fencing_token != envelope.refresh_fencing_token {
                return Err(IntegrationContractError::LeaseFenceMismatch.to_string());
            }
            let current =
                ark_store::integration_replication::load_max_integration_credential_generation(
                    conn,
                    &envelope.integration_id,
                )?;
            if current.is_some_and(|value| envelope.credential_generation < value) {
                return Err(
                    crate::integration_replication::IntegrationContractError::NonMonotonic {
                        field: "credential_generation",
                    }
                    .to_string(),
                );
            }
            if let Some(current) =
                ark_store::integration_replication::load_integration_credential_envelope_record(
                    conn,
                    &envelope.integration_id,
                    &envelope.recipient_node_id,
                    envelope.credential_generation,
                )?
            {
                if !envelope_version_is_newer(
                    &envelope.hlc,
                    &envelope.issuer_node_id,
                    &current.hlc,
                    &current.issuer_node_id,
                ) {
                    return Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string());
                }
            }
            ark_store::integration_replication::upsert_integration_credential_envelope_record(
                conn, envelope,
            )
        },
    )
}

pub fn load_integration_credential_envelope<S: IntegrationReplicationReadPort>(
    conn: &S,
    integration_id: &str,
    recipient_node_id: &str,
    credential_generation: u64,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let Some(envelope) =
        conn.credential_envelope(integration_id, recipient_node_id, credential_generation)?
    else {
        return Ok(None);
    };
    validate_loaded_envelope_recipient(conn, &envelope)?;
    Ok(Some(envelope))
}

pub fn load_latest_integration_credential_envelope<S: IntegrationReplicationReadPort>(
    conn: &S,
    integration_id: &str,
    recipient_node_id: &str,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let Some(envelope) = conn.latest_credential_envelope(integration_id, recipient_node_id)? else {
        return Ok(None);
    };
    validate_loaded_envelope_recipient(conn, &envelope)?;
    Ok(Some(envelope))
}

fn validate_loaded_envelope_recipient<S: IntegrationReplicationReadPort>(
    conn: &S,
    envelope: &IntegrationCredentialEnvelope,
) -> Result<(), String> {
    let node = conn
        .authorized_node(&envelope.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "authorized_node",
            }
            .to_string()
        })?;
    let grant = conn
        .grant(&envelope.integration_id, &envelope.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    validate_envelope_recipient(envelope, &node, &grant).map_err(|error| error.to_string())
}
