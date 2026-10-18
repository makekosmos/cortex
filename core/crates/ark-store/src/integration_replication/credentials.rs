use ark_domain::integration_replication::IntegrationCredentialEnvelope;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

fn u64_value(value: i64, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} is negative"))
}

pub fn load_integration_credential_envelope_record(
    connection: &Connection,
    integration_id: &str,
    recipient_node_id: &str,
    credential_generation: u64,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let generation = i64::try_from(credential_generation)
        .map_err(|_| "credential_generation exceeds SQLite INTEGER range".to_owned())?;
    let row = connection
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
        .map_err(|error| error.to_string())?;
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
        integration_id: integration_id.to_owned(),
        recipient_node_id: recipient_node_id.to_owned(),
        grant_epoch: u64_value(epoch, "grant_epoch")?,
        credential_generation,
        refresh_fencing_token: u64_value(refresh_fence, "refresh_fencing_token")?,
        key_id,
        algorithm,
        nonce,
        ciphertext,
        authenticated_metadata: metadata
            .map(|value| serde_json::from_str::<Value>(&value).map_err(|error| error.to_string()))
            .transpose()?,
        issuer_node_id: issuer,
        issued_at: issued,
        revision: u64_value(revision, "revision")?,
        hlc,
    };
    record.validate().map_err(|error| error.to_string())?;
    Ok(Some(record))
}

pub fn load_latest_integration_credential_envelope_record(
    connection: &Connection,
    integration_id: &str,
    recipient_node_id: &str,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let generation = connection
        .query_row(
            "SELECT MAX(credential_generation) FROM integration_credential_envelopes
             WHERE integration_id = ?1 AND recipient_node_id = ?2",
            params![integration_id, recipient_node_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|error| error.to_string())?;
    generation
        .map(|value| {
            load_integration_credential_envelope_record(
                connection,
                integration_id,
                recipient_node_id,
                u64_value(value, "credential_generation")?,
            )
        })
        .transpose()
        .map(Option::flatten)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_domain::integration_replication::encryption_key_id;

    #[test]
    fn envelope_queries_decode_a_generation_and_its_latest_record() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("envelope schema applies");
        let envelope = IntegrationCredentialEnvelope {
            integration_id: "app".to_owned(),
            recipient_node_id: "device".to_owned(),
            grant_epoch: 2,
            credential_generation: 3,
            refresh_fencing_token: 4,
            key_id: encryption_key_id("recipient-key"),
            algorithm: "test-aead".to_owned(),
            nonce: "nonce".to_owned(),
            ciphertext: "ciphertext".to_owned(),
            authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
            issuer_node_id: "issuer".to_owned(),
            issued_at: "2026-08-30T00:00:00Z".to_owned(),
            revision: 3,
            hlc: "2026-08-30T00:00:00.000Z:000003:issuer".to_owned(),
        };
        connection
            .execute(
                "INSERT INTO integration_credential_envelopes
                 (envelope_id, integration_id, recipient_node_id, grant_epoch,
                  credential_generation, refresh_fencing_token, key_id, algorithm, nonce,
                  ciphertext, authenticated_metadata_json, issuer_node_id, issued_at,
                  revision, hlc)
                 VALUES ('id', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    envelope.integration_id,
                    envelope.recipient_node_id,
                    envelope.grant_epoch,
                    envelope.credential_generation,
                    envelope.refresh_fencing_token,
                    envelope.key_id,
                    envelope.algorithm,
                    envelope.nonce,
                    envelope.ciphertext,
                    serde_json::to_string(&envelope.authenticated_metadata)
                        .expect("metadata serializes"),
                    envelope.issuer_node_id,
                    envelope.issued_at,
                    envelope.revision,
                    envelope.hlc,
                ],
            )
            .expect("fixture envelope inserts");

        assert_eq!(
            load_integration_credential_envelope_record(&connection, "app", "device", 3)
                .expect("generation query succeeds"),
            Some(envelope.clone())
        );
        assert_eq!(
            load_latest_integration_credential_envelope_record(&connection, "app", "device")
                .expect("latest query succeeds"),
            Some(envelope)
        );
        assert!(
            load_latest_integration_credential_envelope_record(&connection, "app", "missing")
                .expect("missing latest query succeeds")
                .is_none()
        );
    }
}
