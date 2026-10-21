#[cfg(test)]
use super::NodeStatus;
pub use ark_application::integration_sync::{
    validate_outbound_signed_sync, validate_outbound_signed_sync_with_transport,
};
#[cfg(test)]
use ark_store::integration_replication::{upsert_authorized_node, upsert_integration_node_grant};
#[cfg(test)]
use rusqlite::Connection;

/// A node-level transport block may be removed only when every persisted
/// integration grant for that node is active and bound to its current key.
#[cfg(test)]
pub fn node_may_receive_integration_frames(
    conn: &Connection,
    node_id: &str,
) -> Result<bool, String> {
    let Some(node) = ark_store::integration_replication::load_authorized_node(conn, node_id)?
    else {
        return Ok(false);
    };
    if node.status != NodeStatus::Active {
        return Ok(false);
    }
    ark_store::integration_replication::node_grants_match_current_authorization(
        conn,
        node_id,
        node.grant_epoch,
        &node.encryption_public_key,
    )
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::integration_replication::{
        encode_hex, AuthorizedNode, GrantStatus, IntegrationConfiguration,
        IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationReplicationChange,
        IntegrationReplicationEntity, SignedSyncEnvelope,
    };

    fn validate_with_store_snapshot(
        connection: &mut rusqlite::Connection,
        frame: &SignedSyncEnvelope,
    ) -> Result<(), String> {
        ark_store::read_snapshot::test_support::with_read_snapshot(connection, |capabilities| {
            validate_outbound_signed_sync(
                &capabilities.integration_replication(),
                "space",
                "origin",
                frame,
            )
        })
        .map_err(|error| match error {
            ark_store::read_snapshot::ReadSnapshotFailure::Operation(error) => error,
            error => error.to_string(),
        })
    }

    #[test]
    fn revoked_grant_invalidates_an_already_signed_frame() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(ark_store::schema::CREATE_TABLES)
            .unwrap();
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
            upsert_authorized_node(&conn, node, "writer").unwrap();
        }
        for grant in [&origin_grant, &recipient_grant] {
            upsert_integration_node_grant(&conn, grant, "writer").unwrap();
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
        validate_with_store_snapshot(&mut conn, &frame).unwrap();
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
        assert!(validate_with_store_snapshot(&mut conn, &forged).is_err());

        let revoked = recipient_grant.revoke(2, "2026-08-31T00:01:00Z").unwrap();
        upsert_integration_node_grant(&conn, &revoked, "writer").unwrap();
        assert!(validate_with_store_snapshot(&mut conn, &frame).is_err());
        let mut other_active = grant("recipient", "recipient-key");
        other_active.integration_id = "other-integration".into();
        upsert_integration_node_grant(&conn, &other_active, "writer").unwrap();
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
