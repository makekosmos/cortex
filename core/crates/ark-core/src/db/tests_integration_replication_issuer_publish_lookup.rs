fn setup_publish_key_authority(conn: &rusqlite::Connection) -> (AuthorizedNode, AuthorizedNode) {
    upsert_integration_configuration(conn, &integration_config(serde_json::json!({})), "node-a")
        .unwrap();
    let recipient = integration_node();
    upsert_authorized_node(conn, &recipient, "node-a").unwrap();
    upsert_integration_node_grant(conn, &integration_grant(), "node-a").unwrap();

    let mut issuer = integration_node();
    issuer.node_id = "node-b".into();
    issuer.key_fingerprint = "fp-b".into();
    issuer.signing_public_key = "sign-b".into();
    issuer.encryption_public_key = "enc-b".into();
    issuer.hlc = "2026-08-30T00:00:00.000Z:000001:node-b".into();
    let mut issuer_grant = integration_grant();
    issuer_grant.node_id = "node-b".into();
    issuer_grant.node_encryption_key = issuer.encryption_public_key.clone();
    issuer_grant.hlc = "2026-08-30T00:00:00.000Z:000001:node-b".into();
    upsert_authorized_node(conn, &issuer, "node-a").unwrap();
    upsert_integration_node_grant(conn, &issuer_grant, "node-a").unwrap();
    (recipient, issuer)
}

#[test]
fn issuer_key_lookup_for_publish_supports_first_publish_without_an_envelope() {
    let conn = integration_test_db();
    let (_recipient, issuer) = setup_publish_key_authority(&conn);
    let expected_key_id = encryption_key_id(&issuer.encryption_public_key);
    let key = load_issuer_encryption_key_for_publish(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        &expected_key_id,
    )
    .unwrap();
    assert_eq!(key.schema_version, 1);
    assert_eq!(key.encryption_public_key, "enc-b");
    assert_eq!(key.key_id, expected_key_id);
    assert_eq!(key.grant_epoch, 2);
    let encoded = serde_json::to_string(&key).unwrap();
    for forbidden in ["credential_generation", "ciphertext", "nonce", "private"] {
        assert!(!encoded.contains(forbidden), "unexpected field: {forbidden}");
    }
}

#[test]
fn issuer_key_lookup_for_publish_rejects_wrong_key_and_accepts_rotated_key() {
    let conn = integration_test_db();
    let (_recipient, issuer) = setup_publish_key_authority(&conn);
    let stale_key_id = encryption_key_id(&issuer.encryption_public_key);
    assert!(load_issuer_encryption_key_for_publish(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        "sha256:stale",
    )
    .unwrap_err()
    .contains("issuer_key_id"));

    let rotated = issuer
        .reauthorize(
            Reauthorization {
                grant_epoch: 3,
                key_fingerprint: "fp-b-rotated".into(),
                signing_public_key: "sign-b-rotated".into(),
                encryption_public_key: "enc-b-rotated".into(),
                transport_public_key: None,
                authorized_at: "2026-08-30T00:00:00Z".into(),
                hlc: "2026-08-30T00:00:00.000Z:000002:node-b".into(),
            },
        )
        .unwrap();
    let mut rotated_grant = integration_grant();
    rotated_grant.node_id = "node-b".into();
    rotated_grant.node_encryption_key = rotated.encryption_public_key.clone();
    rotated_grant.grant_epoch = rotated.grant_epoch;
    rotated_grant.hlc = "2026-08-30T00:00:00.000Z:000002:node-b".into();
    crate::integration_replication::persist_node_authorization(
        &conn,
        crate::integration_replication::NodeAuthorizationOperation::Rotate,
        &rotated,
        Some(&rotated_grant),
        "node-b",
    )
    .unwrap();
    assert!(load_issuer_encryption_key_for_publish(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        &stale_key_id,
    )
    .unwrap_err()
    .contains("issuer_key_id"));
    let current = load_issuer_encryption_key_for_publish(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        &encryption_key_id(&rotated.encryption_public_key),
    )
    .unwrap();
    assert_eq!(current.grant_epoch, 3);
}

#[test]
fn issuer_key_lookup_for_publish_rejects_revoked_authority() {
    let conn = integration_test_db();
    let (recipient, _issuer) = setup_publish_key_authority(&conn);
    let revoked = recipient.revoke(3, "2026-08-30T00:01:00Z").unwrap();
    let revoked_grant = integration_grant().revoke(3, "2026-08-30T00:01:00Z").unwrap();
    crate::integration_replication::persist_node_authorization(
        &conn,
        crate::integration_replication::NodeAuthorizationOperation::Revoke,
        &revoked,
        Some(&revoked_grant),
        "node-a",
    )
    .unwrap();
    let error = load_issuer_encryption_key_for_publish(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        "sha256:issuer",
    )
    .unwrap_err();
    assert_eq!(error, "node is revoked");
}