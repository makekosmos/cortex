use std::collections::BTreeSet;

use rusqlite::{params, Connection};

use super::{IntegrationReplicationEntity, NodeStatus, SignedSyncEnvelope, VerifiedSyncEnvelope};

/// Revalidate a host-signed frame against current authorization immediately
/// before transport routing. A frame prepared before revocation must not send.
pub fn validate_outbound_signed_sync(
    conn: &Connection,
    expected_space_id: &str,
    expected_origin_node_id: &str,
    frame: &SignedSyncEnvelope,
) -> Result<(), String> {
    if frame.origin_node_id != expected_origin_node_id {
        return Err("signed integration frame origin is not this node".into());
    }
    let origin = crate::db::load_authorized_node(conn, expected_origin_node_id)?
        .ok_or_else(|| "origin node is not authorized in this database".to_string())?;
    let verified = frame
        .verify(expected_space_id, &frame.recipient_node_id, &origin)
        .map_err(|error| error.to_string())?;
    let recipient = crate::db::load_authorized_node(conn, &frame.recipient_node_id)?
        .ok_or_else(|| "recipient node is not authorized in this database".to_string())?;
    if recipient.status != NodeStatus::Active {
        return Err("recipient node is revoked".into());
    }

    let integrations = integration_ids(&verified);
    if integrations.is_empty() {
        return Err("signed integration payload has no integration scope".into());
    }
    for integration_id in integrations {
        super::api::require_current_grant(conn, integration_id, &origin)?;
        super::api::require_current_grant(conn, integration_id, &recipient)?;
    }
    for change in &verified.payload {
        match &change.entity {
            IntegrationReplicationEntity::IntegrationRefreshLease(lease)
                if lease.holder_node_id != verified.origin_node_id =>
            {
                return Err("refresh lease holder does not match signed origin".into());
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(envelope) => {
                if envelope.issuer_node_id != verified.origin_node_id {
                    return Err("credential envelope issuer does not match signed origin".into());
                }
                let grant = crate::db::load_integration_node_grant(
                    conn,
                    &envelope.integration_id,
                    &recipient.node_id,
                )?
                .ok_or_else(|| "recipient node has no integration grant".to_string())?;
                super::validate_envelope_recipient(envelope, &recipient, &grant)
                    .map_err(|error| error.to_string())?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn validate_outbound_signed_sync_with_transport(
    conn: &Connection,
    expected_space_id: &str,
    expected_origin_node_id: &str,
    frame: &SignedSyncEnvelope,
    transport_public_key: &str,
) -> Result<(), String> {
    validate_outbound_signed_sync(conn, expected_space_id, expected_origin_node_id, frame)?;
    let recipient = crate::db::load_authorized_node(conn, &frame.recipient_node_id)?
        .ok_or_else(|| "recipient node is not authorized in this database".to_string())?;
    if recipient.transport_public_key.as_deref() != Some(transport_public_key) {
        return Err("recipient transport identity is no longer authorized".into());
    }
    Ok(())
}

/// A node-level transport block may be removed only when every persisted
/// integration grant for that node is active and bound to its current key.
pub fn node_may_receive_integration_frames(
    conn: &Connection,
    node_id: &str,
) -> Result<bool, String> {
    let Some(node) = crate::db::load_authorized_node(conn, node_id)? else {
        return Ok(false);
    };
    if node.status != NodeStatus::Active {
        return Ok(false);
    }
    let epoch = i64::try_from(node.grant_epoch)
        .map_err(|_| "node grant epoch exceeds SQLite INTEGER range".to_string())?;
    let (total, invalid): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE
                WHEN status = 'active' AND grant_epoch = ?2 AND node_encryption_key = ?3
                THEN 0 ELSE 1 END), 0)
             FROM integration_node_grants WHERE node_id = ?1",
            params![node_id, epoch, node.encryption_public_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    Ok(total > 0 && invalid == 0)
}

fn integration_ids(frame: &VerifiedSyncEnvelope) -> BTreeSet<&str> {
    frame
        .payload
        .iter()
        .filter_map(|change| match &change.entity {
            IntegrationReplicationEntity::IntegrationConfiguration(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::IntegrationNodeGrant(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::IntegrationRefreshLease(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
                Some(value.integration_id.as_str())
            }
            IntegrationReplicationEntity::AuthorizedNode(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::integration_replication::{
        encode_hex, AuthorizedNode, GrantStatus, IntegrationConfiguration,
        IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationReplicationChange,
        NodeStatus,
    };

    #[test]
    fn revoked_grant_invalidates_an_already_signed_frame() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::schema::CREATE_TABLES).unwrap();
        let key = SigningKey::from_bytes(&[7; 32]);
        let origin = node(
            "origin",
            "origin-key",
            encode_hex(key.verifying_key().as_bytes()),
        );
        let recipient = node("recipient", "recipient-key", "recipient-signing".into());
        let origin_grant = grant("origin", "origin-key");
        let recipient_grant = grant("recipient", "recipient-key");
        for node in [&origin, &recipient] {
            crate::db::upsert_authorized_node(&conn, node, "writer").unwrap();
        }
        for grant in [&origin_grant, &recipient_grant] {
            crate::db::upsert_integration_node_grant(&conn, grant, "writer").unwrap();
        }

        let mut frame = SignedSyncEnvelope::new(
            "space",
            "origin",
            "recipient",
            1,
            "message",
            vec![IntegrationReplicationChange {
                entity: IntegrationReplicationEntity::IntegrationConfiguration(
                    IntegrationConfiguration {
                        integration_id: "integration".into(),
                        provider: "fatsecret".into(),
                        account_subject: "account".into(),
                        public_scopes: vec![],
                        public_settings: serde_json::json!({}),
                        enabled: true,
                        sync_cursor: None,
                        revision: 1,
                        hlc: hlc("origin"),
                    },
                ),
                vector_hlc: hlc("origin"),
            }],
            "",
        );
        frame.signature = encode_hex(
            &key.sign(&frame.canonical_signing_bytes().unwrap())
                .to_bytes(),
        );
        validate_outbound_signed_sync(&conn, "space", "origin", &frame).unwrap();
        assert!(node_may_receive_integration_frames(&conn, "recipient").unwrap());

        let mut forged = frame.clone();
        forged.payload.push(IntegrationReplicationChange {
            entity: IntegrationReplicationEntity::IntegrationCredentialEnvelope(
                IntegrationCredentialEnvelope {
                    integration_id: "integration".into(),
                    recipient_node_id: "recipient".into(),
                    grant_epoch: 1,
                    credential_generation: 1,
                    refresh_fencing_token: 1,
                    key_id: crate::integration_replication::encryption_key_id("recipient-key"),
                    algorithm: "xchacha20poly1305".into(),
                    nonce: "nonce".into(),
                    ciphertext: "opaque".into(),
                    authenticated_metadata: None,
                    issuer_node_id: "other-node".into(),
                    issued_at: "2026-08-31T00:00:00Z".into(),
                    revision: 1,
                    hlc: hlc("other-node"),
                },
            ),
            vector_hlc: hlc("other-node"),
        });
        forged.signature = encode_hex(
            &key.sign(&forged.canonical_signing_bytes().unwrap())
                .to_bytes(),
        );
        assert!(validate_outbound_signed_sync(&conn, "space", "origin", &forged).is_err());

        let revoked = recipient_grant.revoke(2, "2026-08-31T00:01:00Z").unwrap();
        crate::db::upsert_integration_node_grant(&conn, &revoked, "writer").unwrap();
        assert!(validate_outbound_signed_sync(&conn, "space", "origin", &frame).is_err());
        let mut other_active = grant("recipient", "recipient-key");
        other_active.integration_id = "other-integration".into();
        crate::db::upsert_integration_node_grant(&conn, &other_active, "writer").unwrap();
        assert!(!node_may_receive_integration_frames(&conn, "recipient").unwrap());
    }

    fn node(node_id: &str, encryption_key: &str, signing_key: String) -> AuthorizedNode {
        AuthorizedNode {
            node_id: node_id.into(),
            key_fingerprint: format!("fingerprint-{node_id}"),
            signing_public_key: signing_key,
            encryption_public_key: encryption_key.into(),
            transport_public_key: None,
            grant_epoch: 1,
            status: NodeStatus::Active,
            authorized_at: "2026-08-31T00:00:00Z".into(),
            revoked_at: None,
            revocation_epoch: None,
            revision: 1,
            hlc: hlc(node_id),
        }
    }

    fn grant(node_id: &str, encryption_key: &str) -> IntegrationNodeGrant {
        IntegrationNodeGrant {
            integration_id: "integration".into(),
            node_id: node_id.into(),
            node_encryption_key: encryption_key.into(),
            grant_epoch: 1,
            status: GrantStatus::Active,
            authorized_at: "2026-08-31T00:00:00Z".into(),
            revoked_at: None,
            revision: 1,
            hlc: hlc(node_id),
        }
    }

    fn hlc(node_id: &str) -> String {
        format!("2026-08-31T00:00:00.000Z:000001:{node_id}")
    }
}
