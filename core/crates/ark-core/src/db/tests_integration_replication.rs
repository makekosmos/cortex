use crate::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationCredentialEnvelope,
    IntegrationNodeGrant, IntegrationRefreshLease, NodeStatus,
};

fn integration_test_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::schema::CREATE_TABLES).unwrap();
    conn
}

fn integration_config(settings: serde_json::Value) -> IntegrationConfiguration {
    IntegrationConfiguration {
        integration_id: "integration-a".into(),
        provider: "fatsecret".into(),
        account_subject: "account-a".into(),
        public_scopes: vec!["diary.read".into()],
        public_settings: settings,
        enabled: true,
        sync_cursor: Some(1),
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn integration_node() -> AuthorizedNode {
    AuthorizedNode {
        node_id: "node-a".into(),
        key_fingerprint: "fp-a".into(),
        signing_public_key: "sign-a".into(),
        encryption_public_key: "enc-a".into(),
        transport_public_key: None,
        grant_epoch: 2,
        status: NodeStatus::Active,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}
fn integration_grant() -> IntegrationNodeGrant {
    IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: "node-a".into(),
        node_encryption_key: "enc-a".into(),
        grant_epoch: 2,
        status: GrantStatus::Active,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: None,
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn integration_envelope(generation: u64) -> IntegrationCredentialEnvelope {
    IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-a".into(),
        grant_epoch: 2,
        credential_generation: generation,
        refresh_fencing_token: 1,
        key_id: crate::integration_replication::encryption_key_id("enc-a"),
        algorithm: "opaque-test".into(),
        nonce: format!("nonce-{generation}"),
        ciphertext: format!("opaque-ciphertext-{generation}"),
        authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
        issuer_node_id: "node-a".into(),
        issued_at: "2026-08-30T00:00:00Z".into(),
        revision: generation,
        hlc: format!("2026-08-30T00:00:00.000Z:00000{generation}:node-a"),
    }
}

fn acquire_integration_test_lease(
    conn: &rusqlite::Connection,
    device_id: &str,
) -> IntegrationRefreshLease {
    try_acquire_integration_refresh_lease(conn, "integration-a", "node-a", 3, 10, 10, 0, device_id)
        .unwrap()
}

#[test]
fn verification_status_is_plaintext_free_and_reports_local_readiness() {
    let conn = integration_test_db();
    let empty = integration_verification_status(&conn, "integration-a", "node-a", 15).unwrap();
    assert!(!empty.integration_present);
    assert!(!empty.ready_for_collection);
    assert!(!empty.ready_for_refresh);

    upsert_integration_configuration(
        &conn,
        &integration_config(serde_json::json!({"display": {"region": "us"}})),
        "node-a",
    )
    .unwrap();
    upsert_authorized_node(&conn, &integration_node(), "node-a").unwrap();
    upsert_integration_node_grant(&conn, &integration_grant(), "node-a").unwrap();
    acquire_integration_test_lease(&conn, "node-a");
    publish_integration_credential_envelope(&conn, &integration_envelope(3), "node-a", 15).unwrap();

    let status = integration_verification_status(&conn, "integration-a", "node-a", 15).unwrap();
    assert!(status.integration_present);
    assert_eq!(status.integration_enabled, Some(true));
    assert_eq!(status.node_status, Some(NodeStatus::Active));
    assert_eq!(status.grant_status, Some(GrantStatus::Active));
    assert!(status.grant_key_matches_node);
    assert_eq!(status.envelope_generation, Some(3));
    assert!(status.envelope_available);
    assert!(status.ready_for_collection);
    assert!(!status.ready_for_refresh);
    assert!(
        integration_verification_status(&conn, "integration-a", "node-a", 20)
            .unwrap()
            .ready_for_refresh
    );
    let encoded = serde_json::to_string(&status).unwrap();
    for forbidden in [
        "opaque-ciphertext",
        "nonce",
        "authenticated_metadata",
        "public_settings",
        "region",
    ] {
        assert!(!encoded.contains(forbidden));
    }
}
