use super::*;
use serde_json::{json, Value};

fn configuration(settings: Value) -> IntegrationConfiguration {
    IntegrationConfiguration {
        integration_id: "integration-a".into(),
        provider: "fatsecret".into(),
        account_subject: "account-a".into(),
        public_scopes: vec!["diary.read".into()],
        public_settings: settings,
        enabled: true,
        sync_cursor: None,
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn node(status: NodeStatus) -> AuthorizedNode {
    AuthorizedNode {
        node_id: "node-a".into(),
        key_fingerprint: "fp-a".into(),
        signing_public_key: "sign-a".into(),
        encryption_public_key: "enc-a".into(),
        transport_public_key: None,
        grant_epoch: 2,
        status,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: (status == NodeStatus::Revoked).then(|| "2026-08-30T01:00:00Z".into()),
        revocation_epoch: (status == NodeStatus::Revoked).then_some(2),
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn grant(status: GrantStatus) -> IntegrationNodeGrant {
    IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: "node-a".into(),
        node_encryption_key: "enc-a".into(),
        grant_epoch: 2,
        status,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: (status == GrantStatus::Revoked).then(|| "2026-08-30T01:00:00Z".into()),
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn envelope() -> IntegrationCredentialEnvelope {
    IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-a".into(),
        grant_epoch: 2,
        credential_generation: 3,
        refresh_fencing_token: 1,
        key_id: encryption_key_id("enc-a"),
        algorithm: "opaque-test".into(),
        nonce: "nonce-a".into(),
        ciphertext: "opaque-ciphertext-only".into(),
        authenticated_metadata: Some(json!({"content_type": "oauth"})),
        issuer_node_id: "node-a".into(),
        issued_at: "2026-08-30T00:00:00Z".into(),
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000003:node-a".into(),
    }
}

#[test]
fn nested_secret_shaped_public_configuration_is_rejected() {
    for field in ["refresh_token", "passphrase", "credential_key"] {
        let error = configuration(json!({
            "display": {"provider": "fatsecret"},
            "nested": [{"oauth": {field: "plaintext"}}]
        }))
        .validate()
        .expect_err("nested credential-shaped keys must fail closed");
        assert!(matches!(
            error,
            IntegrationContractError::SensitiveField { .. }
        ));
    }
}

#[test]
fn opaque_sync_cursor_cannot_enter_public_configuration() {
    let raw = serde_json::json!({
        "integration_id": "integration-a",
        "provider": "fatsecret",
        "account_subject": "account-a",
        "public_scopes": [],
        "public_settings": {},
        "enabled": true,
        "sync_cursor": "opaque-provider-token",
        "revision": 1,
        "hlc": "2026-08-30T00:00:00.000Z:000001:node-a"
    });
    assert!(serde_json::from_value::<IntegrationConfiguration>(raw).is_err());
}

#[test]
fn public_contracts_reject_plaintext_shaped_unknown_fields() {
    for field in [
        "password",
        "access_token",
        "refresh_token",
        "client_secret",
        "private_key",
        "passphrase",
    ] {
        let mut configuration = serde_json::to_value(configuration(json!({}))).unwrap();
        configuration
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("plaintext"));
        assert!(serde_json::from_value::<IntegrationConfiguration>(configuration).is_err());

        let mut envelope = serde_json::to_value(envelope()).unwrap();
        envelope
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("plaintext"));
        assert!(serde_json::from_value::<IntegrationCredentialEnvelope>(envelope).is_err());

        let mut frame = serde_json::to_value(SignedSyncEnvelope::new(
            "space-a",
            "node-a",
            "node-b",
            1,
            "message-a",
            Vec::new(),
            "",
        ))
        .unwrap();
        frame
            .as_object_mut()
            .unwrap()
            .insert(field.into(), json!("plaintext"));
        assert!(serde_json::from_value::<SignedSyncEnvelope>(frame).is_err());
    }
}

#[test]
fn credential_envelope_hlc_origin_must_match_issuer() {
    let mut value = envelope();
    value.hlc = "2026-08-30T00:00:00.000Z:000003:node-b".into();
    assert!(matches!(
        value.validate(),
        Err(IntegrationContractError::Mismatch {
            field: "hlc_origin"
        })
    ));
}

#[test]
fn credential_envelope_hlc_must_be_canonical() {
    let mut value = envelope();
    value.hlc = "not-a-time:000003:node-a".into();
    assert!(matches!(
        value.validate(),
        Err(IntegrationContractError::Mismatch { field: "hlc" })
    ));
}

#[test]
fn opaque_sync_cursor_cannot_enter_public_configuration_or_replication() {
    let raw = serde_json::json!({
        "integration_id": "integration-a",
        "provider": "fatsecret",
        "account_subject": "account-a",
        "public_scopes": [],
        "public_settings": {},
        "enabled": true,
        "sync_cursor": "opaque-provider-token",
        "revision": 1,
        "hlc": "2026-08-30T00:00:00.000Z:000001:node-a"
    });
    assert!(serde_json::from_value::<IntegrationConfiguration>(raw).is_err());

    let mut numeric = configuration(json!({"display": {"region": "us"}}));
    numeric.sync_cursor = Some(42);
    assert_eq!(numeric.sync_cursor, Some(42));
    let encoded = serde_json::to_value(numeric).unwrap();
    assert_eq!(encoded.get("sync_cursor"), Some(&serde_json::json!(42)));
}

#[test]
fn envelope_recipient_and_grant_are_verified() {
    validate_envelope_recipient(
        &envelope(),
        &node(NodeStatus::Active),
        &grant(GrantStatus::Active),
    )
    .expect("matching recipient/grant should validate");

    let mut wrong = envelope();
    wrong.recipient_node_id = "node-b".into();
    assert!(matches!(
        validate_envelope_recipient(
            &wrong,
            &node(NodeStatus::Active),
            &grant(GrantStatus::Active)
        ),
        Err(IntegrationContractError::Mismatch {
            field: "recipient_node_id"
        })
    ));

    let mut stale_key = envelope();
    stale_key.key_id = encryption_key_id("retired-enc-a");
    assert!(matches!(
        validate_envelope_recipient(
            &stale_key,
            &node(NodeStatus::Active),
            &grant(GrantStatus::Active)
        ),
        Err(IntegrationContractError::Mismatch { field: "key_id" })
    ));
}

#[test]
fn revocation_and_reauthorization_require_a_higher_epoch() {
    let revoked = node(NodeStatus::Active)
        .revoke(3, "2026-08-30T01:00:00Z")
        .unwrap();
    assert_eq!(revoked.grant_epoch, 3);
    assert!(matches!(
        validate_envelope_recipient(&envelope(), &revoked, &grant(GrantStatus::Active)),
        Err(IntegrationContractError::RevokedNode)
    ));
    assert!(matches!(
        revoked.reauthorize(
            3,
            "fp-b",
            "sign-b",
            "enc-b",
            None,
            "2026-08-30T02:00:00Z",
            "2026-08-30T02:00:00.000Z:000001:node-b",
        ),
        Err(IntegrationContractError::NonMonotonic {
            field: "grant_epoch"
        })
    ));
    let reauthorized = revoked
        .reauthorize(
            4,
            "fp-b",
            "sign-b",
            "enc-b",
            Some("iroh-endpoint-b".into()),
            "2026-08-30T02:00:00Z",
            "2026-08-30T02:00:00.000Z:000001:node-b",
        )
        .unwrap();
    assert_eq!(reauthorized.grant_epoch, 4);
    assert_eq!(
        reauthorized.transport_public_key.as_deref(),
        Some("iroh-endpoint-b")
    );

    let revoked_grant = grant(GrantStatus::Active)
        .revoke(3, "2026-08-30T01:00:00Z")
        .unwrap();
    assert_eq!(revoked_grant.grant_epoch, 3);
    assert!(matches!(
        validate_envelope_recipient(&envelope(), &node(NodeStatus::Active), &revoked_grant),
        Err(IntegrationContractError::RevokedGrant)
    ));
}

include!("tests_part02.rs");
