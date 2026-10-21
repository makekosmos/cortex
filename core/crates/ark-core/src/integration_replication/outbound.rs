pub use ark_application::integration_sync::{
    validate_outbound_signed_sync, validate_outbound_signed_sync_with_transport,
};

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::integration_replication::{
        AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationCredentialEnvelope,
        IntegrationNodeGrant, IntegrationReplicationChange, IntegrationReplicationEntity,
        NodeStatus, SignedSyncEnvelope, encode_hex,
    };
    use ark_store::{read_snapshot::SqliteReader, test_support::TestDatabase};

    fn validate_with_store_snapshot(
        reader: &mut SqliteReader,
        frame: &SignedSyncEnvelope,
    ) -> Result<(), String> {
        reader
            .with_read_snapshot(|capabilities| {
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
        let database = TestDatabase::create().unwrap();
        let path = database.path();
        let key = SigningKey::from_bytes(&[7; 32]);
        let origin = node(
            "origin",
            "origin-key",
            encode_hex(key.verifying_key().as_bytes()),
        );
        let recipient = node("recipient", "recipient-key", "recipient-signing".into());
        let origin_grant = grant("origin", "origin-key");
        let recipient_grant = grant("recipient", "recipient-key");
        ark_store::test_support::IntegrationReplicationTestFixture::seed_authorization(
            path,
            &[origin, recipient],
            &[origin_grant, recipient_grant.clone()],
            "writer",
        )
        .unwrap();
        let path_text = path.to_str().unwrap();
        let mut reader = SqliteReader::open_read_only(path_text).unwrap();

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
        validate_with_store_snapshot(&mut reader, &frame).unwrap();

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
        assert!(validate_with_store_snapshot(&mut reader, &forged).is_err());

        let revoked = recipient_grant.revoke(2, "2026-08-31T00:01:00Z").unwrap();
        ark_store::test_support::IntegrationReplicationTestFixture::upsert_grant(
            path, &revoked, "writer",
        )
        .unwrap();
        assert!(validate_with_store_snapshot(&mut reader, &frame).is_err());
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
