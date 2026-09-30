use crate::integration_replication::Reauthorization;
fn setup_issuer_key_lookup(conn: &rusqlite::Connection) -> AuthorizedNode {
    upsert_integration_configuration(conn, &integration_config(serde_json::json!({})), "node-a")
        .unwrap();
    upsert_authorized_node(conn, &integration_node(), "node-a").unwrap();
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

    let lease = try_acquire_integration_refresh_lease(
        conn,
        &RefreshLeaseAcquireParams {
            integration_id: "integration-a".into(),
            holder_node_id: "node-b".into(),
            credential_generation: 3,
            now_ms: 10,
            ttl_ms: 10,
            expected_fencing_token: 0,
            device_id: "node-b".into(),
        },
    )
    .unwrap();
    let mut envelope = integration_envelope(lease.credential_generation);
    envelope.issuer_node_id = issuer.node_id.clone();
    envelope.refresh_fencing_token = lease.fencing_token;
    envelope.hlc = "2026-08-30T00:00:00.000Z:000002:node-b".into();
    publish_integration_credential_envelope(conn, &envelope, "node-b", 15).unwrap();
    issuer
}

#[test]
fn issuer_key_lookup_returns_current_public_material_only() {
    let conn = integration_test_db();
    let issuer = setup_issuer_key_lookup(&conn);
    let expected_key_id = encryption_key_id(&issuer.encryption_public_key);

    let key = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        3,
        &expected_key_id,
    )
    .unwrap();
    assert_eq!(key.schema_version, 1);
    assert_eq!(key.space_id, "space-a");
    assert_eq!(key.integration_id, "integration-a");
    assert_eq!(key.recipient_node_id, "node-a");
    assert_eq!(key.issuer_node_id, "node-b");
    assert_eq!(key.encryption_public_key, "enc-b");
    assert_eq!(key.key_id, expected_key_id);
    assert_eq!(key.status, NodeStatus::Active);
    assert_eq!(key.grant_status, GrantStatus::Active);
    assert_eq!(key.grant_epoch, 2);
    assert_eq!(key.credential_generation, 3);

    let encoded = serde_json::to_string(&key).unwrap();
    for forbidden in ["ciphertext", "nonce", "authenticated_metadata", "opaque"] {
        assert!(!encoded.contains(forbidden), "unexpected secret field: {forbidden}");
    }
}

#[test]
fn issuer_key_lookup_rejects_a_retained_older_generation() {
    let conn = integration_test_db();
    let issuer = setup_issuer_key_lookup(&conn);
    let expected_key_id = encryption_key_id(&issuer.encryption_public_key);
    let next_lease = try_acquire_integration_refresh_lease(
        &conn,
        &RefreshLeaseAcquireParams {
            integration_id: "integration-a".into(),
            holder_node_id: "node-b".into(),
            credential_generation: 4,
            now_ms: 20,
            ttl_ms: 10,
            expected_fencing_token: 1,
            device_id: "node-b".into(),
        },
    )
    .unwrap();
    let mut latest = integration_envelope(4);
    latest.issuer_node_id = issuer.node_id.clone();
    latest.refresh_fencing_token = next_lease.fencing_token;
    latest.hlc = "2026-08-30T00:00:00.000Z:000004:node-b".into();
    publish_integration_credential_envelope(&conn, &latest, "node-b", 25).unwrap();

    let older_error = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        3,
        &expected_key_id,
    )
    .unwrap_err();
    assert!(older_error.contains("credential_generation"));
    let current = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        4,
        &expected_key_id,
    )
    .unwrap();
    assert_eq!(current.credential_generation, 4);
}

#[test]
fn issuer_key_lookup_rejects_wrong_binding_rotation_and_revocation() {
    let conn = integration_test_db();
    let issuer = setup_issuer_key_lookup(&conn);
    let expected_key_id = encryption_key_id(&issuer.encryption_public_key);

    let wrong_issuer = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-a",
        3,
        &expected_key_id,
    )
    .unwrap_err();
    assert!(wrong_issuer.contains("issuer_node_id"));

    let stale_key = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        3,
        "sha256:stale",
    )
    .unwrap_err();
    assert!(stale_key.contains("issuer_key_id"));

    let rotated = issuer
        .reauthorize(
            Reauthorization {
                grant_epoch: 3,
                key_fingerprint: "fp-b-rotated".into(),
                signing_public_key: "sign-b-rotated".into(),
                encryption_public_key: "enc-b-rotated".into(),
                transport_public_key: None,
                authorized_at: "2026-08-30T00:00:00Z".into(),
                hlc: "2026-08-30T00:00:00.000Z:000003:node-b".into(),
            },
        )
        .unwrap();
    let mut rotated_grant = integration_grant();
    rotated_grant.node_id = "node-b".into();
    rotated_grant.node_encryption_key = rotated.encryption_public_key.clone();
    rotated_grant.grant_epoch = rotated.grant_epoch;
    rotated_grant.hlc = "2026-08-30T00:00:00.000Z:000003:node-b".into();
    crate::integration_replication::persist_node_authorization(
        &conn,
        crate::integration_replication::NodeAuthorizationOperation::Rotate,
        &rotated,
        Some(&rotated_grant),
        "node-b",
    )
    .unwrap();
    let rotated_error = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        3,
        &expected_key_id,
    )
    .unwrap_err();
    assert!(rotated_error.contains("issuer_key_id"));

    let revoked = rotated.revoke(4, "2026-08-30T00:01:00Z").unwrap();
    let revoked_grant = rotated_grant.revoke(4, "2026-08-30T00:01:00Z").unwrap();
    crate::integration_replication::persist_node_authorization(
        &conn,
        crate::integration_replication::NodeAuthorizationOperation::Revoke,
        &revoked,
        Some(&revoked_grant),
        "node-b",
    )
    .unwrap();
    let revoked_error = load_issuer_encryption_key(
        &conn,
        "space-a",
        "integration-a",
        "node-a",
        "node-b",
        3,
        &encryption_key_id(&rotated.encryption_public_key),
    )
    .unwrap_err();
    assert_eq!(revoked_error, "node is revoked");
}