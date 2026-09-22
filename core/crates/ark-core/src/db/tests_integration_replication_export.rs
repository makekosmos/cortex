use crate::integration_replication::IntegrationReplicationEntity;

#[test]
fn peer_export_is_typed_ordered_and_filters_addressed_envelopes() {
    let conn = integration_test_db();
    let config = integration_config(serde_json::json!({"display": {"region": "us"}}));
    let node_a = integration_node();
    let grant_a = integration_grant();
    upsert_integration_configuration(&conn, &config, "device-a").unwrap();
    upsert_authorized_node(&conn, &node_a, "device-a").unwrap();
    upsert_integration_node_grant(&conn, &grant_a, "device-a").unwrap();
    acquire_integration_test_lease(&conn, "node-a");
    publish_integration_credential_envelope(&conn, &integration_envelope(3), "node-a", 15)
        .unwrap();

    let node_b = AuthorizedNode {
        node_id: "node-b".into(),
        key_fingerprint: "fp-b".into(),
        signing_public_key: "sign-b".into(),
        encryption_public_key: "enc-b".into(),
        ..node_a.clone()
    };
    let grant_b = IntegrationNodeGrant {
        node_id: "node-b".into(),
        node_encryption_key: "enc-b".into(),
        ..grant_a.clone()
    };
    upsert_authorized_node(&conn, &node_b, "device-a").unwrap();
    upsert_integration_node_grant(&conn, &grant_b, "device-a").unwrap();

    let changes = export_integration_replication(&conn, "integration-a", "node-a").unwrap();
    assert!(changes
        .windows(2)
        .all(|pair| rank(&pair[0].entity) <= rank(&pair[1].entity)));
    let envelopes = changes
        .iter()
        .filter_map(|change| match &change.entity {
            IntegrationReplicationEntity::IntegrationCredentialEnvelope(envelope) => Some(envelope),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0].recipient_node_id, "node-a");
    assert_eq!(envelopes[0].ciphertext, "opaque-ciphertext-3");
    assert!(!changes.iter().any(|change| matches!(
        change.entity,
        IntegrationReplicationEntity::AuthorizedNode(_)
    )));
    assert!(!changes.iter().any(|change| matches!(
        change.entity,
        IntegrationReplicationEntity::IntegrationNodeGrant(_)
    )));
    assert!(changes.iter().all(|change| !change.vector_hlc.is_empty()));
}

fn rank(entity: &IntegrationReplicationEntity) -> u8 {
    match entity {
        IntegrationReplicationEntity::IntegrationConfiguration(_) => 0,
        IntegrationReplicationEntity::AuthorizedNode(_) => 1,
        IntegrationReplicationEntity::IntegrationNodeGrant(_) => 2,
        IntegrationReplicationEntity::IntegrationRefreshLease(_) => 3,
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(_) => 4,
    }
}

#[test]
fn peer_export_sends_no_envelope_to_revoked_or_mismatched_recipient() {
    let conn = integration_test_db();
    let config = integration_config(serde_json::json!({}));
    let node = integration_node();
    let grant = integration_grant();
    upsert_integration_configuration(&conn, &config, "device-a").unwrap();
    upsert_authorized_node(&conn, &node, "device-a").unwrap();
    upsert_integration_node_grant(&conn, &grant, "device-a").unwrap();
    acquire_integration_test_lease(&conn, "node-a");
    publish_integration_credential_envelope(&conn, &integration_envelope(3), "node-a", 15)
        .unwrap();
    let revoked = node.revoke(3, "2026-08-30T01:00:00Z").unwrap();
    upsert_authorized_node(&conn, &revoked, "device-a").unwrap();
    let changes = export_integration_replication(&conn, "integration-a", "node-a").unwrap();
    assert!(!changes.iter().any(|change| matches!(
        change.entity,
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(_)
    )));
    assert!(!changes.iter().any(|change| matches!(
        change.entity,
        IntegrationReplicationEntity::IntegrationRefreshLease(_)
    )));
}
