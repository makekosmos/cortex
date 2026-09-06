use crate::integration_replication::IssuerEncryptionKey;

pub fn load_issuer_encryption_key(
    conn: &Connection,
    space_id: &str,
    integration_id: &str,
    recipient_node_id: &str,
    issuer_node_id: &str,
    credential_generation: u64,
    expected_issuer_key_id: &str,
) -> Result<IssuerEncryptionKey, String> {
    for (value, field) in [
        (space_id, "space_id"),
        (integration_id, "integration_id"),
        (recipient_node_id, "recipient_node_id"),
        (issuer_node_id, "issuer_node_id"),
        (expected_issuer_key_id, "expected_issuer_key_id"),
    ] {
        if value.trim().is_empty() {
            return Err(IntegrationContractError::EmptyField { field }.to_string());
        }
    }
    let envelope = load_integration_credential_envelope(
        conn,
        integration_id,
        recipient_node_id,
        credential_generation,
    )?
    .ok_or_else(|| {
        IntegrationContractError::Mismatch {
            field: "credential_envelope",
        }
        .to_string()
    })?;
    let latest = load_latest_integration_credential_envelope(conn, integration_id, recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "credential_envelope",
            }
            .to_string()
        })?;
    if latest.credential_generation != credential_generation {
        return Err(IntegrationContractError::Mismatch {
            field: "credential_generation",
        }
        .to_string());
    }
    if envelope.issuer_node_id != issuer_node_id {
        return Err(IntegrationContractError::Mismatch {
            field: "issuer_node_id",
        }
        .to_string());
    }
    let issuer = load_authorized_node(conn, issuer_node_id)?.ok_or_else(|| {
        IntegrationContractError::Mismatch {
            field: "issuer_authorized_node",
        }
        .to_string()
    })?;
    let issuer_grant = load_integration_node_grant(conn, integration_id, issuer_node_id)?
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
    let key_id = encryption_key_id(&issuer.encryption_public_key);
    if key_id != expected_issuer_key_id {
        return Err(IntegrationContractError::Mismatch {
            field: "issuer_key_id",
        }
        .to_string());
    }
    Ok(IssuerEncryptionKey {
        schema_version: 1,
        space_id: space_id.to_string(),
        integration_id: integration_id.to_string(),
        recipient_node_id: recipient_node_id.to_string(),
        issuer_node_id: issuer_node_id.to_string(),
        encryption_public_key: issuer.encryption_public_key,
        key_id,
        status: issuer.status,
        grant_status: issuer_grant.status,
        grant_epoch: issuer.grant_epoch,
        credential_generation,
    })
}
