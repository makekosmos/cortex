use super::*;
use serde_json::Value;

fn context(recipient: &HpkeIdentity, issuer: &HpkeIdentity) -> CredentialContext {
    CredentialContext {
        integration_id: "com.example.provider".into(),
        recipient_node_id: recipient.node_id.clone(),
        grant_epoch: 7,
        credential_generation: 3,
        key_id: recipient.key_id.clone(),
        issuer_node_id: issuer.node_id.clone(),
        issuer_auth_key_id: issuer.key_id.clone(),
    }
}

#[test]
fn auth_round_trip_and_keyring_store() {
    let recipient = load_or_create_identity("recipient-v2").unwrap();
    let issuer = load_or_create_identity("issuer-v2").unwrap();
    let context = context(&recipient, &issuer);
    let envelope = encrypt(&context, &recipient.public_key, &issuer, "oauth-secret").unwrap();
    let encoded = serde_json::to_string(&envelope).unwrap();
    assert!(!encoded.contains("oauth-secret"));
    assert_eq!(
        decrypt(&envelope, &context, &recipient, &issuer.public_key)
            .unwrap()
            .as_str(),
        "oauth-secret"
    );
    decrypt_and_store(
        &envelope,
        &context,
        &recipient,
        &issuer.public_key,
        "com.example.provider",
        "1.0.0",
        "session",
    )
    .unwrap();
    assert_eq!(
        read_package_integration_secret("com.example.provider", "1.0.0", "session").as_deref(),
        Some("oauth-secret")
    );
}

#[test]
fn context_tampering_fails_closed() {
    let recipient = load_or_create_identity("recipient-negative").unwrap();
    let issuer = load_or_create_identity("issuer-negative").unwrap();
    let context = context(&recipient, &issuer);
    let envelope = encrypt(&context, &recipient.public_key, &issuer, "secret").unwrap();
    for mutate in [
        |value: &mut CredentialEnvelopeV2| value.key_id.push('x'),
        |value: &mut CredentialEnvelopeV2| value.recipient_node_id.push('x'),
        |value: &mut CredentialEnvelopeV2| value.grant_epoch += 1,
        |value: &mut CredentialEnvelopeV2| value.credential_generation += 1,
        |value: &mut CredentialEnvelopeV2| value.issuer_node_id.push('x'),
    ] {
        let mut tampered = envelope.clone();
        mutate(&mut tampered);
        assert!(decrypt(&tampered, &context, &recipient, &issuer.public_key).is_err());
    }
}

#[test]
fn wrong_keys_and_noncanonical_encoding_fail_closed() {
    let recipient = load_or_create_identity("recipient-wrong-key").unwrap();
    let wrong_recipient = load_or_create_identity("wrong-recipient").unwrap();
    let issuer = load_or_create_identity("issuer-wrong-key").unwrap();
    let wrong_issuer = load_or_create_identity("wrong-issuer").unwrap();
    let context = context(&recipient, &issuer);
    let envelope = encrypt(&context, &recipient.public_key, &issuer, "secret").unwrap();
    assert!(decrypt(&envelope, &context, &wrong_recipient, &issuer.public_key).is_err());
    assert!(decrypt(&envelope, &context, &recipient, &wrong_issuer.public_key).is_err());

    let mut noncanonical = envelope;
    noncanonical.enc.push('=');
    assert!(matches!(
        decrypt(&noncanonical, &context, &recipient, &issuer.public_key),
        Err(CredentialEnvelopeError::InvalidEncoding)
    ));
}

#[test]
fn version_and_algorithm_are_strict() {
    let recipient = load_or_create_identity("recipient-version").unwrap();
    let issuer = load_or_create_identity("issuer-version").unwrap();
    let context = context(&recipient, &issuer);
    let envelope = encrypt(&context, &recipient.public_key, &issuer, "secret").unwrap();
    let mut wrong_version = envelope.clone();
    wrong_version.version = 1;
    assert!(decrypt(&wrong_version, &context, &recipient, &issuer.public_key).is_err());
    let mut wrong_algorithm = envelope;
    wrong_algorithm.algorithm = "HPKE-Base".into();
    assert!(decrypt(&wrong_algorithm, &context, &recipient, &issuer.public_key).is_err());
}

#[test]
fn core_opaque_shape_is_admitted_only_for_v2_algorithm() {
    let recipient = load_or_create_identity("recipient-core-shape").unwrap();
    let issuer = load_or_create_identity("issuer-core-shape").unwrap();
    let context = context(&recipient, &issuer);
    let envelope = encrypt(&context, &recipient.public_key, &issuer, "secret").unwrap();
    let core_value = serde_json::json!({
        "integration_id": envelope.integration_id,
        "recipient_node_id": envelope.recipient_node_id,
        "grant_epoch": envelope.grant_epoch,
        "credential_generation": envelope.credential_generation,
        "refresh_fencing_token": 1,
        "key_id": envelope.key_id,
        "algorithm": envelope.algorithm,
        "nonce": envelope.enc,
        "ciphertext": envelope.ciphertext,
        "issuer_node_id": envelope.issuer_node_id,
        "issued_at": "2026-09-06T00:00:00Z",
        "revision": 1,
        "hlc": "2026-09-06T00:00:00.000Z:issuer-core-shape"
    });
    let parsed = CredentialEnvelopeV2::from_core_value(&core_value, &issuer.key_id).unwrap();
    assert_eq!(
        decrypt(&parsed, &context, &recipient, &issuer.public_key)
            .unwrap()
            .as_str(),
        "secret"
    );
    let mut legacy = core_value;
    legacy["algorithm"] = Value::String("opaque-test".into());
    assert!(CredentialEnvelopeV2::from_core_value(&legacy, &issuer.key_id).is_err());
}
