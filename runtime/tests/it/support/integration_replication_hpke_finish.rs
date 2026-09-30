use super::*;

pub async fn receive_and_validate(
    marker: &Path,
    setup: &support::IntegrationReplicationSetup,
    issuer: &engine::package_service::credential_envelope::HpkeIdentity,
    recipient_host: &ArkHost,
    recipient_dir: &Path,
    recipient_packages: &mut PackageService,
) {
    let received = handle_operation(
        "replication_receive_credential_envelope_v2",
        json!({
            "space_id": SPACE_ID,
            "integration_id": PACKAGE_ID,
            "recipient_node_id": setup.recipient.node_id,
            "issuer_node_id": setup.origin.node_id,
            "credential_generation": 1,
            "issuer_auth_key_id": issuer.key_id,
            "package_version": "forged-version-is-ignored",
            "setting": "forged-setting-is-ignored",
        }),
        recipient_host,
        recipient_dir,
        recipient_packages,
    )
    .await
    .unwrap();
    assert_eq!(received["stored"], true);
    let stored = keyring::Entry::new(
        "kosmos-kepler",
        &format!("package-integration:{PACKAGE_ID}:{PACKAGE_VERSION}:{SETTING}"),
    )
    .unwrap()
    .get_password()
    .unwrap();
    assert_eq!(stored, "replicated-provider-secret");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if marker.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let provider_result = std::fs::read_to_string(marker).unwrap();
    assert!(provider_result.contains("replicated-item"));

    let wrong_space = handle_operation(
        "replication_receive_credential_envelope_v2",
        json!({
            "space_id": "wrong-space",
            "integration_id": PACKAGE_ID,
            "recipient_node_id": setup.recipient.node_id,
            "issuer_node_id": setup.origin.node_id,
            "credential_generation": 1,
            "issuer_auth_key_id": issuer.key_id,
        }),
        recipient_host,
        recipient_dir,
        recipient_packages,
    )
    .await;
    assert!(wrong_space.is_err());

    let stale = handle_operation(
        "replication_receive_credential_envelope_v2",
        json!({
            "space_id": SPACE_ID,
            "integration_id": PACKAGE_ID,
            "recipient_node_id": setup.recipient.node_id,
            "issuer_node_id": setup.origin.node_id,
            "credential_generation": 2,
            "issuer_auth_key_id": issuer.key_id,
        }),
        recipient_host,
        recipient_dir,
        recipient_packages,
    )
    .await;
    assert!(stale.is_err());
    let wrong_recipient = handle_operation(
        "replication_receive_credential_envelope_v2",
        json!({
            "space_id": SPACE_ID,
            "integration_id": PACKAGE_ID,
            "recipient_node_id": setup.foreign.node_id,
            "issuer_node_id": setup.origin.node_id,
            "credential_generation": 1,
            "issuer_auth_key_id": issuer.key_id,
        }),
        recipient_host,
        recipient_dir,
        recipient_packages,
    )
    .await;
    assert!(wrong_recipient.is_err());

    let conn = rusqlite::Connection::open(&setup.recipient_db).unwrap();
    let mut revoked_node = serde_json::to_value(
        ark_core::db::load_authorized_node(&conn, &setup.recipient.node_id)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    revoked_node["grant_epoch"] = json!(3);
    revoked_node["status"] = json!("revoked");
    revoked_node["revoked_at"] = json!("2026-09-07T00:00:01Z");
    revoked_node["revocation_epoch"] = json!(3);
    revoked_node["revision"] = json!(3);
    revoked_node["hlc"] = json!("2026-09-07T00:00:01.000Z:000001:recipient-node");
    let mut revoked_grant = serde_json::to_value(
        ark_core::db::load_integration_node_grant(&conn, PACKAGE_ID, &setup.recipient.node_id)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    revoked_grant["grant_epoch"] = json!(3);
    revoked_grant["status"] = json!("revoked");
    revoked_grant["revoked_at"] = json!("2026-09-07T00:00:01Z");
    revoked_grant["revision"] = json!(3);
    revoked_grant["hlc"] = json!("2026-09-07T00:00:01.000Z:000001:recipient-node");
    let revoked = recipient_host
        .request(
            "integration.persist_node_authorization",
            json!({
                "authorization_operation": "revoke",
                "node": revoked_node,
                "grant": revoked_grant,
                "device_id": setup.recipient.node_id,
            }),
        )
        .await
        .unwrap();
    assert!(
        revoked.ok,
        "recipient revocation failed: {:?}",
        revoked.error
    );
    let revoked_receive = handle_operation(
        "replication_receive_credential_envelope_v2",
        json!({
            "space_id": SPACE_ID,
            "integration_id": PACKAGE_ID,
            "recipient_node_id": setup.recipient.node_id,
            "issuer_node_id": setup.origin.node_id,
            "credential_generation": 1,
            "issuer_auth_key_id": issuer.key_id,
        }),
        recipient_host,
        recipient_dir,
        recipient_packages,
    )
    .await;
    assert!(revoked_receive.is_err());
    recipient_packages
        .set_enabled(PACKAGE_ID, PACKAGE_VERSION, false)
        .await
        .unwrap();
}
