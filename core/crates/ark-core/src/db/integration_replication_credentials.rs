use crate::integration_replication::{
    encryption_key_id, validate_envelope_recipient, GrantStatus, IntegrationCredentialEnvelope,
};

fn upsert_integration_credential_envelope_record(
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
    let epoch = sqlite_i64(envelope.grant_epoch, "grant_epoch")?;
    let generation = sqlite_i64(envelope.credential_generation, "credential_generation")?;
    let refresh_fence = sqlite_i64(envelope.refresh_fencing_token, "refresh_fencing_token")?;
    let revision = sqlite_i64(envelope.revision, "revision")?;
    let metadata = envelope
        .authenticated_metadata
        .as_ref()
        .map(json)
        .transpose()?;
    persist_with_vector(
        conn,
        "integration_credential_envelope",
        &envelope_id,
        device_id,
        || {
            let node =
                load_authorized_node(conn, &envelope.recipient_node_id)?.ok_or_else(|| {
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
            let issuer =
                load_authorized_node(conn, &envelope.issuer_node_id)?.ok_or_else(|| {
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
            let lease = load_integration_refresh_lease(conn, &envelope.integration_id)?
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
            let current = conn
                .query_row(
                    "SELECT MAX(credential_generation)
                 FROM integration_credential_envelopes WHERE integration_id = ?1",
                    params![envelope.integration_id],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .map_err(storage)?;
            if current.is_some_and(|value| generation < value) {
                return Err(
                    crate::integration_replication::IntegrationContractError::NonMonotonic {
                        field: "credential_generation",
                    }
                    .to_string(),
                );
            }
            if let Some((hlc, issuer)) = conn
                .query_row(
                    "SELECT hlc, issuer_node_id FROM integration_credential_envelopes
                 WHERE envelope_id = ?1",
                    params![envelope_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(storage)?
            {
                if !envelope_version_is_newer(
                    &envelope.hlc,
                    &envelope.issuer_node_id,
                    &hlc,
                    &issuer,
                ) {
                    return Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string());
                }
            }
            conn.execute(
                "INSERT INTO integration_credential_envelopes
             (envelope_id, integration_id, recipient_node_id, grant_epoch,
              credential_generation, refresh_fencing_token, key_id, algorithm, nonce, \
              ciphertext,
              authenticated_metadata_json, issuer_node_id, issued_at, revision, hlc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(envelope_id) DO UPDATE SET
                integration_id = excluded.integration_id,
                recipient_node_id = excluded.recipient_node_id,
                grant_epoch = excluded.grant_epoch,
                credential_generation = excluded.credential_generation,
                refresh_fencing_token = excluded.refresh_fencing_token,
                key_id = excluded.key_id,
                algorithm = excluded.algorithm,
                nonce = excluded.nonce,
                ciphertext = excluded.ciphertext,
                authenticated_metadata_json = excluded.authenticated_metadata_json,
                issuer_node_id = excluded.issuer_node_id,
                issued_at = excluded.issued_at,
                revision = excluded.revision,
                hlc = excluded.hlc",
                params![
                    envelope_id,
                    envelope.integration_id,
                    envelope.recipient_node_id,
                    epoch,
                    generation,
                    refresh_fence,
                    envelope.key_id,
                    envelope.algorithm,
                    envelope.nonce,
                    envelope.ciphertext,
                    metadata,
                    envelope.issuer_node_id,
                    envelope.issued_at,
                    revision,
                    envelope.hlc,
                ],
            )
            .map_err(storage)?;
            Ok(())
        },
    )
}

pub fn load_integration_credential_envelope(
    conn: &Connection,
    integration_id: &str,
    recipient_node_id: &str,
    credential_generation: u64,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let generation = sqlite_i64(credential_generation, "credential_generation")?;
    let row = conn
        .query_row(
            "SELECT grant_epoch, refresh_fencing_token, key_id, algorithm, nonce, ciphertext,
                    authenticated_metadata_json, issuer_node_id, issued_at,
                    revision, hlc FROM integration_credential_envelopes
             WHERE integration_id = ?1 AND recipient_node_id = ?2
               AND credential_generation = ?3",
            params![integration_id, recipient_node_id, generation],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((
        epoch,
        refresh_fence,
        key_id,
        algorithm,
        nonce,
        ciphertext,
        metadata,
        issuer,
        issued,
        revision,
        hlc,
    )) = row
    else {
        return Ok(None);
    };
    let record = IntegrationCredentialEnvelope {
        integration_id: integration_id.to_string(),
        recipient_node_id: recipient_node_id.to_string(),
        grant_epoch: u64_value(epoch, "grant_epoch")?,
        credential_generation,
        refresh_fencing_token: u64_value(refresh_fence, "refresh_fencing_token")?,
        key_id,
        algorithm,
        nonce,
        ciphertext,
        authenticated_metadata: metadata.map(|value| parse::<Value>(&value)).transpose()?,
        issuer_node_id: issuer,
        issued_at: issued,
        revision: u64_value(revision, "revision")?,
        hlc,
    };
    record.validate().map_err(|e| e.to_string())?;
    let node = load_authorized_node(conn, recipient_node_id)?.ok_or_else(|| {
        IntegrationContractError::Mismatch {
            field: "authorized_node",
        }
        .to_string()
    })?;
    let grant =
        load_integration_node_grant(conn, integration_id, recipient_node_id)?.ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    validate_envelope_recipient(&record, &node, &grant).map_err(|error| error.to_string())?;
    Ok(Some(record))
}

pub fn load_latest_integration_credential_envelope(
    conn: &Connection,
    integration_id: &str,
    recipient_node_id: &str,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let generation = conn
        .query_row(
            "SELECT MAX(credential_generation) FROM integration_credential_envelopes
             WHERE integration_id = ?1 AND recipient_node_id = ?2",
            params![integration_id, recipient_node_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(storage)?;
    generation
        .map(|value| {
            load_integration_credential_envelope(
                conn,
                integration_id,
                recipient_node_id,
                u64_value(value, "credential_generation")?,
            )
        })
        .transpose()
        .map(Option::flatten)
}
