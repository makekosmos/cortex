use crate::integration_replication::{IssuerEncryptionKey, IssuerEncryptionKeyForPublish};

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
        refresh_fencing_token: envelope.refresh_fencing_token,
    })
}

/// The expected issuer binding and refresh lease a replicated credential was
/// minted under — one value instead of seven loose parameters.
pub struct CredentialFenceExpectation<'a> {
    pub space_id: &'a str,
    pub integration_id: &'a str,
    pub recipient_node_id: &'a str,
    pub issuer_node_id: &'a str,
    pub credential_generation: u64,
    pub refresh_fencing_token: u64,
    pub expected_issuer_key_id: &'a str,
}

/// Storage-time fence for `replication_receive_credential_envelope_v2`: the
/// issuer binding is revalidated through `load_issuer_encryption_key`, then
/// the fence the envelope was minted under must still be the current refresh
/// lease for this integration — same holder, generation and fencing token.
/// A lease that moved on (newer fence, different holder, next generation)
/// means the replicated credential was superseded before it was stored.
pub fn check_integration_credential_fence(
    conn: &Connection,
    expected: &CredentialFenceExpectation<'_>,
) -> Result<(), String> {
    let key = load_issuer_encryption_key(
        conn,
        expected.space_id,
        expected.integration_id,
        expected.recipient_node_id,
        expected.issuer_node_id,
        expected.credential_generation,
        expected.expected_issuer_key_id,
    )?;
    if key.refresh_fencing_token != expected.refresh_fencing_token {
        return Err(IntegrationContractError::Mismatch {
            field: "refresh_fencing_token",
        }
        .to_string());
    }
    let lease =
        load_integration_refresh_lease(conn, expected.integration_id)?.ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "refresh_lease",
            }
            .to_string()
        })?;
    if lease.holder_node_id != expected.issuer_node_id
        || lease.credential_generation != expected.credential_generation
        || lease.fencing_token != expected.refresh_fencing_token
    {
        return Err(IntegrationContractError::NonMonotonic {
            field: "fencing_token",
        }
        .to_string());
    }
    Ok(())
}

pub fn load_issuer_encryption_key_for_publish(
    conn: &Connection,
    space_id: &str,
    integration_id: &str,
    recipient_node_id: &str,
    issuer_node_id: &str,
    expected_issuer_key_id: &str,
) -> Result<IssuerEncryptionKeyForPublish, String> {
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
    if load_integration_configuration(conn, integration_id)?.is_none() {
        return Err(IntegrationContractError::Mismatch {
            field: "integration_configuration",
        }
        .to_string());
    }
    let recipient = load_authorized_node(conn, recipient_node_id)?.ok_or_else(|| {
        IntegrationContractError::Mismatch {
            field: "recipient_authorized_node",
        }
        .to_string()
    })?;
    let recipient_grant = load_integration_node_grant(conn, integration_id, recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "recipient_integration_node_grant",
            }
            .to_string()
        })?;
    if recipient.status != NodeStatus::Active {
        return Err(IntegrationContractError::RevokedNode.to_string());
    }
    if recipient_grant.status != GrantStatus::Active {
        return Err(IntegrationContractError::RevokedGrant.to_string());
    }
    if recipient.grant_epoch != recipient_grant.grant_epoch
        || recipient.encryption_public_key != recipient_grant.node_encryption_key
    {
        return Err(IntegrationContractError::Mismatch {
            field: "recipient_node_key_epoch",
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
    Ok(IssuerEncryptionKeyForPublish {
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
    })
}
