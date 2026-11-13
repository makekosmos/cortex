use ed25519_dalek::{SigningKey, VerifyingKey};

use super::*;

fn rename_draft(sequence: u64, previous_event_hash: Option<String>) -> RegistryEventDraft {
    let actor = DeviceId::parse("device-a").unwrap();
    RegistryEventDraft {
        actor_device_id: actor.clone(),
        actor_sequence: sequence,
        previous_event_hash,
        target_device_id: DeviceId::parse("device-b").unwrap(),
        actor_key_epoch: 1,
        actor_admission_epoch: 1,
        hlc: crate::hlc::HLC::new(
            "2026-10-16T12:00:00.000Z".into(),
            sequence,
            actor.as_str().into(),
        )
        .to_string(),
        operation: RegistryOperation::Rename {
            display_name: DisplayName::parse("Office Mac").unwrap(),
        },
    }
}

#[test]
fn signed_registry_event_verifies_and_hashes_canonically() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let actor = DeviceId::parse("device-a").unwrap();
    let event = RegistryEvent::sign(rename_draft(1, None), &key).unwrap();

    let verified = event.verify(&actor, 1, &key.verifying_key()).unwrap();
    assert_eq!(event.event_id, "device-a:1");
    assert_eq!(
        event.canonical_signing_bytes().unwrap().first_chunk::<32>(),
        Some(b"cortex-device-registry-event-v1\0")
    );
    assert_eq!(
        event.signature,
        concat!(
            "d36a4331e34f23663b7869840196df13aa76246263c64558fd4c02fc5e7883b4",
            "e731e8c2c9f3918b31ef28c3c5d1144112f1e36dad95cb2fd088c4a3ccc79300"
        )
    );
    assert_eq!(
        verified.event_hash().unwrap(),
        "b2e6dd270a14c692a7123aced9e917b1f1f38b6e09cc7d58f70886f8d446b3f8"
    );
}

#[test]
fn signature_binds_every_registry_field() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let actor = DeviceId::parse("device-a").unwrap();
    let mut event = RegistryEvent::sign(rename_draft(1, None), &key).unwrap();
    event.target_device_id = DeviceId::parse("device-c").unwrap();

    assert_eq!(
        event.verify(&actor, 1, &key.verifying_key()),
        Err(RegistryEventError::InvalidSignature)
    );
}

#[test]
fn event_requires_the_enrolled_actor_and_key_epoch() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let event = RegistryEvent::sign(rename_draft(1, None), &key).unwrap();

    assert_eq!(
        event.verify(
            &DeviceId::parse("device-c").unwrap(),
            1,
            &key.verifying_key()
        ),
        Err(RegistryEventError::ActorMismatch)
    );
    assert_eq!(
        event.verify(
            &DeviceId::parse("device-a").unwrap(),
            2,
            &key.verifying_key()
        ),
        Err(RegistryEventError::KeyEpochMismatch)
    );
}

#[test]
fn sequence_chain_and_actor_hlc_are_validated_before_signing() {
    let key = SigningKey::from_bytes(&[7; 32]);
    assert!(matches!(
        RegistryEvent::sign(rename_draft(2, None), &key),
        Err(RegistryEventError::InvalidField {
            field: "previous_event_hash"
        })
    ));

    let mut draft = rename_draft(1, None);
    draft.hlc =
        crate::hlc::HLC::new("2026-10-16T12:00:00.000Z".into(), 1, "device-c".into()).to_string();
    assert!(matches!(
        RegistryEvent::sign(draft, &key),
        Err(RegistryEventError::InvalidField { field: "hlc" })
    ));
}

#[test]
fn display_names_are_trimmed_bounded_and_control_free() {
    assert!(DisplayName::parse(" ").is_err());
    assert!(DisplayName::parse(" Office Mac").is_err());
    assert!(DisplayName::parse("Office\nMac").is_err());
    assert!(DisplayName::parse("x".repeat(MAX_DISPLAY_NAME_BYTES + 1)).is_err());
    assert_eq!(
        DisplayName::parse("Office Mac").unwrap().as_str(),
        "Office Mac"
    );
}

#[test]
fn public_key_constructor_rejects_invalid_ed25519_points() {
    let invalid = SigningPublicKey::from_bytes([0; 32]);
    assert!(invalid.is_err());
    let key = SigningKey::from_bytes(&[5; 32]);
    assert_eq!(
        SigningPublicKey::from_bytes(*key.verifying_key().as_bytes())
            .unwrap()
            .as_bytes(),
        key.verifying_key().as_bytes()
    );
}

#[test]
fn valid_key_bytes_can_be_used_by_the_verifier() {
    let key = SigningKey::from_bytes(&[9; 32]);
    let public = SigningPublicKey::from_bytes(*key.verifying_key().as_bytes()).unwrap();
    let verifier = VerifyingKey::from_bytes(public.as_bytes()).unwrap();
    let event = RegistryEvent::sign(rename_draft(1, None), &key).unwrap();

    event
        .verify(&DeviceId::parse("device-a").unwrap(), 1, &verifier)
        .unwrap();
}

#[test]
fn signed_event_roundtrips_through_json_without_bypassing_verification() {
    let key = SigningKey::from_bytes(&[11; 32]);
    let event = RegistryEvent::sign(rename_draft(1, None), &key).unwrap();
    let encoded = serde_json::to_string(&event).unwrap();
    let decoded: RegistryEvent = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, event);
    decoded
        .verify(
            &DeviceId::parse("device-a").unwrap(),
            1,
            &key.verifying_key(),
        )
        .unwrap();
}
