use super::*;
use ed25519_dalek::{Signer, SigningKey};

fn node(key: &SigningKey, status: NodeStatus, epoch: u64) -> AuthorizedNode {
    AuthorizedNode {
        node_id: "node-a".into(),
        key_fingerprint: "fp-a".into(),
        signing_public_key: encode_hex(key.verifying_key().as_bytes()),
        encryption_public_key: "enc-a".into(),
        transport_public_key: None,
        grant_epoch: epoch,
        status,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: (status == NodeStatus::Revoked).then(|| "2026-08-30T01:00:00Z".into()),
        revocation_epoch: (status == NodeStatus::Revoked).then_some(epoch),
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn payload() -> Vec<IntegrationReplicationChange> {
    vec![IntegrationReplicationChange {
        entity: IntegrationReplicationEntity::IntegrationConfiguration(IntegrationConfiguration {
            integration_id: "integration-a".into(),
            provider: "fatsecret".into(),
            account_subject: "account-a".into(),
            public_scopes: vec!["diary.read".into()],
            public_settings: serde_json::json!({"region": "us"}),
            enabled: true,
            sync_cursor: None,
            revision: 1,
            hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
        }),
        vector_hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }]
}

fn signed(key: &SigningKey, origin: &str, epoch: u64) -> SignedSyncEnvelope {
    let mut envelope = SignedSyncEnvelope::new(
        "space-a",
        origin,
        "node-b",
        epoch,
        "message-a",
        payload(),
        "",
    );
    envelope.signature = encode_hex(
        &key.sign(&envelope.canonical_signing_bytes().unwrap())
            .to_bytes(),
    );
    envelope
}

#[test]
fn signed_sync_binds_space_origin_epoch_payload_and_replay() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let active = node(&key, NodeStatus::Active, 2);
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::schema::CREATE_TABLES).unwrap();
    let frame = signed(&key, "node-a", 2);

    verify_reserve_and_apply_signed_sync(
        &conn,
        &frame,
        "space-a",
        "node-b",
        &active,
        10,
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(matches!(
        verify_reserve_and_apply_signed_sync(
            &conn,
            &frame,
            "space-a",
            "node-b",
            &active,
            11,
            |_, _| Ok(())
        ),
        Err(SignedSyncError::Replay)
    ));

    assert!(matches!(
        frame.verify("space-b", "node-b", &active),
        Err(SignedSyncError::WrongSpace)
    ));
    let mut forged = frame.clone();
    forged.signature = encode_hex(
        &SigningKey::from_bytes(&[8; 32])
            .sign(&forged.canonical_signing_bytes().unwrap())
            .to_bytes(),
    );
    assert!(matches!(
        forged.verify("space-a", "node-b", &active),
        Err(SignedSyncError::SignatureMismatch)
    ));
    let mut tampered_payload = frame.clone();
    tampered_payload.payload[0].vector_hlc = "2026-08-30T00:00:00.000Z:000002:node-a".into();
    assert!(matches!(
        tampered_payload.verify("space-a", "node-b", &active),
        Err(SignedSyncError::SignatureMismatch)
    ));
    let forged_origin = signed(&key, "node-b", 2);
    assert!(matches!(
        forged_origin.verify("space-a", "node-b", &active),
        Err(SignedSyncError::OriginMismatch)
    ));
    let mut forged_recipient = frame.clone();
    forged_recipient.recipient_node_id = "node-c".into();
    assert!(matches!(
        forged_recipient.verify("space-a", "node-b", &active),
        Err(SignedSyncError::RecipientMismatch)
    ));

    let stale = signed(&key, "node-a", 1);
    assert!(matches!(
        stale.verify("space-a", "node-b", &active),
        Err(SignedSyncError::StaleEpoch)
    ));
    let revoked = node(&key, NodeStatus::Revoked, 2);
    assert!(matches!(
        frame.verify("space-a", "node-b", &revoked),
        Err(SignedSyncError::RevokedNode)
    ));

    let mut failed = signed(&key, "node-a", 2);
    failed.message_id = "message-b".into();
    failed.signature = encode_hex(
        &key.sign(&failed.canonical_signing_bytes().unwrap())
            .to_bytes(),
    );
    assert!(matches!(
        verify_reserve_and_apply_signed_sync(
            &conn,
            &failed,
            "space-a",
            "node-b",
            &active,
            12,
            |_, _| { Err::<(), _>("apply failed".into()) }
        ),
        Err(SignedSyncError::Apply(_))
    ));
    verify_reserve_and_apply_signed_sync(
        &conn,
        &failed,
        "space-a",
        "node-b",
        &active,
        13,
        |_, _| Ok(()),
    )
    .unwrap();
}
