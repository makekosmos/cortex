use ed25519_dalek::{Signer, SigningKey};

use crate::integration_replication::{encode_hex, SignedSyncEnvelope, SignedSyncError};

fn replication_node(
    node_id: &str,
    key: &SigningKey,
    encryption_key: &str,
    hlc_origin: &str,
) -> AuthorizedNode {
    AuthorizedNode {
        node_id: node_id.into(),
        key_fingerprint: format!("fingerprint-{node_id}"),
        signing_public_key: encode_hex(key.verifying_key().as_bytes()),
        encryption_public_key: encryption_key.into(),
        transport_public_key: None,
        grant_epoch: 2,
        status: NodeStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: format!("2026-08-31T00:00:00.000Z:000001:{hlc_origin}"),
    }
}

fn replication_grant(
    node_id: &str,
    encryption_key: &str,
    grant_epoch: u64,
) -> IntegrationNodeGrant {
    IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: node_id.into(),
        node_encryption_key: encryption_key.into(),
        grant_epoch,
        status: GrantStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revision: 1,
        hlc: format!("2026-08-31T00:00:00.000Z:000002:{node_id}"),
    }
}

fn sign_replication_batch(
    key: &SigningKey,
    key_epoch: u64,
    message_id: &str,
    changes: Vec<crate::integration_replication::IntegrationReplicationChange>,
) -> SignedSyncEnvelope {
    let mut frame = SignedSyncEnvelope::new(
        "space-a", "node-a", "node-b", key_epoch, message_id, changes, "",
    );
    frame.signature = encode_hex(
        &key.sign(&frame.canonical_signing_bytes().unwrap())
            .to_bytes(),
    );
    frame
}

#[test]
fn signed_integration_batch_moves_opaque_envelope_between_independent_databases() {
    let source = integration_test_db();
    let target = integration_test_db();
    let origin_key = SigningKey::from_bytes(&[21; 32]);
    let recipient_key = SigningKey::from_bytes(&[22; 32]);
    let origin = replication_node("node-a", &origin_key, "enc-a", "node-a");
    let recipient = replication_node("node-b", &recipient_key, "enc-b", "node-a");
    let config = integration_config(serde_json::json!({"display": {"region": "us"}}));
    let origin_grant = IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: "node-a".into(),
        node_encryption_key: "enc-a".into(),
        grant_epoch: 2,
        status: GrantStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revision: 1,
        hlc: "2026-08-31T00:00:00.000Z:000002:node-a".into(),
    };
    let grant = IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: "node-b".into(),
        node_encryption_key: "enc-b".into(),
        grant_epoch: 2,
        status: GrantStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revision: 1,
        hlc: "2026-08-31T00:00:00.000Z:000002:node-a".into(),
    };
    let envelope = IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-b".into(),
        grant_epoch: 2,
        credential_generation: 3,
        refresh_fencing_token: 1,
        key_id: crate::integration_replication::encryption_key_id("enc-b"),
        algorithm: "opaque-test".into(),
        nonce: "nonce-3".into(),
        ciphertext: "opaque-ciphertext-only".into(),
        authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
        issuer_node_id: "node-a".into(),
        issued_at: "2026-08-31T00:00:00Z".into(),
        revision: 3,
        hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
    };

    upsert_authorized_node(&source, &origin, "node-a").unwrap();
    upsert_authorized_node(&source, &recipient, "node-a").unwrap();
    upsert_integration_configuration(&source, &config, "node-a").unwrap();
    upsert_integration_node_grant(&source, &origin_grant, "node-a").unwrap();
    upsert_integration_node_grant(&source, &grant, "node-a").unwrap();
    acquire_integration_test_lease(&source, "node-a");
    publish_test_envelope(&source, &envelope, "node-a", 15).unwrap();
    upsert_authorized_node(&target, &origin, "bootstrap").unwrap();
    upsert_authorized_node(&target, &recipient, "bootstrap").unwrap();
    upsert_integration_node_grant(&target, &origin_grant, "bootstrap").unwrap();
    upsert_integration_node_grant(&target, &grant, "bootstrap").unwrap();
    let target_vector = VersionVector::from([
        (
            "authorized_node:node-a".into(),
            "2000-01-01T00:00:00.000Z:000001:bootstrap".into(),
        ),
        (
            "authorized_node:node-b".into(),
            "2000-01-01T00:00:00.000Z:000001:bootstrap".into(),
        ),
    ]);
    replace_version_vector_for_test(&target, &target_vector);

    let changes = with_integration_replication_read(&source, |read| {
        export_integration_replication(read, "integration-a", "node-b")
    })
    .unwrap();
    let expected_vectors = changes
        .iter()
        .map(|change| (change.vector_key(), change.vector_hlc.clone()))
        .collect::<Vec<_>>();
    let vector_repair_changes = changes.clone();
    let frame = sign_replication_batch(&origin_key, 2, "message-two-db", changes);
    drop(source);
    target.fixture.admit_peer_for_test("node-a").unwrap();
    apply_signed_integration_changes(&target.fixture, &frame, "space-a", "node-b", None, 15)
        .unwrap();

    let loaded = integration_read_snapshot(&target)
        .configuration("integration-a")
        .unwrap();
    assert_eq!(loaded, Some(config.clone()));
    assert_eq!(
        integration_read_snapshot(&target)
            .authorized_node("node-b")
            .unwrap(),
        Some(recipient)
    );
    assert_eq!(
        load_integration_node_grant(&target, "integration-a", "node-b").unwrap(),
        Some(grant)
    );
    assert_eq!(
        with_integration_replication_read(&target, |read| {
            load_integration_credential_envelope(read, "integration-a", "node-b", 3)
        })
        .unwrap(),
        Some(envelope)
    );
    let target_vector = current_version_vector(&target);
    for (key, hlc) in &expected_vectors {
        let merged = target_vector.get(key).expect("missing merged typed vector");
        assert!(
            !HLC::is_newer(hlc, merged),
            "typed vector regressed for {key}"
        );
    }

    let (missing_key, expected_hlc) = expected_vectors.last().unwrap();
    let mut missing_vector = target_vector;
    missing_vector.remove(missing_key);
    replace_version_vector_for_test(&target, &missing_vector);
    let repair_frame = sign_replication_batch(
        &origin_key,
        2,
        "message-vector-repair",
        vector_repair_changes,
    );
    target.fixture.admit_peer_for_test("node-a").unwrap();
    apply_signed_integration_changes(
        &target.fixture,
        &repair_frame,
        "space-a",
        "node-b",
        None,
        16,
    )
    .unwrap();
    let repaired_vector = current_version_vector(&target);
    assert_eq!(repaired_vector.get(missing_key), Some(expected_hlc));

    let consumed = with_integration_replication_read(&target, |read| {
        load_integration_credential_envelope(read, "integration-a", "node-b", 3)
    })
    .unwrap()
    .expect("recipient must retain its addressed opaque envelope after source shutdown");
    assert_eq!(consumed.ciphertext, "opaque-ciphertext-only");

    let successor_lease = acquire_test_lease(&target, "node-b", 4, 20, 10, 1, "node-b");
    assert_eq!(successor_lease.fencing_token, 2);
    let refreshed = IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-b".into(),
        grant_epoch: 2,
        credential_generation: 4,
        refresh_fencing_token: successor_lease.fencing_token,
        key_id: crate::integration_replication::encryption_key_id("enc-b"),
        algorithm: "opaque-test".into(),
        nonce: "nonce-4".into(),
        ciphertext: "opaque-refreshed-ciphertext-only".into(),
        authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
        issuer_node_id: "node-b".into(),
        issued_at: "2026-08-31T00:00:01Z".into(),
        revision: 4,
        hlc: "2026-08-31T00:00:01.000Z:000004:node-b".into(),
    };
    publish_test_envelope(&target, &refreshed, "node-b", 21).unwrap();
    assert_eq!(
        with_integration_replication_read(&target, |read| {
            load_integration_credential_envelope(read, "integration-a", "node-b", 4)
        })
        .unwrap(),
        Some(refreshed)
    );
    let mut continued_config = config;
    continued_config.sync_cursor = Some(4);
    continued_config.revision = 2;
    continued_config.hlc = "2026-08-31T00:00:02.000Z:000001:node-b".into();
    upsert_integration_configuration(&target, &continued_config, "node-b").unwrap();
    assert_eq!(
        integration_read_snapshot(&target)
            .configuration("integration-a")
            .unwrap()
            .unwrap()
            .sync_cursor,
        Some(4)
    );
    let target_vector = current_version_vector(&target);
    let configuration_vector = target_vector
        .get("integration_configuration:integration-a")
        .expect("local configuration write must advance its typed vector");
    assert!(
        !HLC::is_newer(&continued_config.hlc, configuration_vector),
        "typed vector must not predate the persisted configuration"
    );
    assert!(target_vector.contains_key("integration_refresh_lease:integration-a"));
    assert!(target_vector.contains_key(&format!(
        "integration_credential_envelope:{}",
        envelope_entity_id("integration-a", "node-b", 4)
    )));

    target.fixture.admit_peer_for_test("node-a").unwrap();
    assert!(matches!(
        apply_signed_integration_changes(&target.fixture, &frame, "space-a", "node-b", None, 16),
        Err(SignedSyncError::Replay)
    ));
}
