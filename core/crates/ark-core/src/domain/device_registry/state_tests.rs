use super::*;
use crate::domain::device_registry::RegistryEventDraft;
use ed25519_dalek::SigningKey;

fn trust_anchor(device_id: &str, key: &SigningKey) -> DeviceRecord {
    DeviceRecord {
        device_id: DeviceId::parse(device_id).unwrap(),
        signing_public_key: SigningPublicKey::from_bytes(*key.verifying_key().as_bytes()).unwrap(),
        key_epoch: 1,
        admission_epoch: 1,
        display_name: None,
        disabled_event_id: None,
        display_name_hlc: None,
        updated_hlc: None,
        updated_event_id: None,
    }
}

fn event(
    actor: &str,
    sequence: u64,
    previous_event_hash: Option<String>,
    target: &str,
    operation: RegistryOperation,
) -> RegistryEventDraft {
    let actor_device_id = DeviceId::parse(actor).unwrap();
    RegistryEventDraft {
        actor_device_id: actor_device_id.clone(),
        actor_sequence: sequence,
        previous_event_hash,
        target_device_id: DeviceId::parse(target).unwrap(),
        actor_key_epoch: 1,
        actor_admission_epoch: 1,
        hlc: crate::hlc::HLC::new(
            "2026-10-16T12:00:00.000Z".into(),
            sequence,
            actor_device_id.as_str().into(),
        )
        .to_string(),
        operation,
    }
}

#[test]
fn enrolled_device_can_rename_itself_after_a_contiguous_enrollment_event() {
    let local_key = SigningKey::from_bytes(&[1; 32]);
    let remote_key = SigningKey::from_bytes(&[2; 32]);
    let local_id = DeviceId::parse("device-a").unwrap();
    let remote_id = DeviceId::parse("device-b").unwrap();
    let mut registry = DeviceRegistry::default();
    registry.add_trust_anchor(trust_anchor("device-a", &local_key));

    let enrollment = RegistryEvent::sign(
        event(
            "device-a",
            1,
            None,
            "device-b",
            RegistryOperation::Enroll {
                signing_public_key: SigningPublicKey::from_bytes(
                    *remote_key.verifying_key().as_bytes(),
                )
                .unwrap(),
                key_epoch: 1,
                display_name: None,
            },
        ),
        &local_key,
    )
    .unwrap();
    let RegistryApplyOutcome::Applied { event_hash } = registry.apply(enrollment).unwrap();

    let rename = RegistryEvent::sign(
        event(
            "device-a",
            2,
            Some(event_hash),
            "device-a",
            RegistryOperation::Rename {
                display_name: DisplayName::parse("Office Mac").unwrap(),
            },
        ),
        &local_key,
    )
    .unwrap();
    registry.apply(rename).unwrap();

    assert_eq!(
        registry.device(&local_id).unwrap().display_name,
        Some(DisplayName::parse("Office Mac").unwrap())
    );
    assert_eq!(registry.device(&remote_id).unwrap().key_epoch, 1);
    assert_eq!(registry.head(&local_id).unwrap().actor_sequence, 2);
}

#[test]
fn registry_rejects_unknown_authors_and_sequence_gaps() {
    let local_key = SigningKey::from_bytes(&[3; 32]);
    let unknown_key = SigningKey::from_bytes(&[4; 32]);
    let mut registry = DeviceRegistry::default();
    registry.add_trust_anchor(trust_anchor("device-a", &local_key));
    let unknown_author = RegistryEvent::sign(
        event(
            "device-b",
            1,
            None,
            "device-a",
            RegistryOperation::Rename {
                display_name: DisplayName::parse("Unknown").unwrap(),
            },
        ),
        &unknown_key,
    )
    .unwrap();
    assert_eq!(
        registry.apply(unknown_author),
        Err(RegistryApplyError::UnknownActor)
    );

    let gap = RegistryEvent::sign(
        event(
            "device-a",
            2,
            Some("a".repeat(64)),
            "device-a",
            RegistryOperation::Rename {
                display_name: DisplayName::parse("Office Mac").unwrap(),
            },
        ),
        &local_key,
    )
    .unwrap();
    assert_eq!(
        registry.apply(gap),
        Err(RegistryApplyError::SequenceGap {
            expected: 1,
            received: 2
        })
    );
}

#[test]
fn concurrent_renames_converge_independent_of_delivery_order() {
    let key_a = SigningKey::from_bytes(&[5; 32]);
    let key_b = SigningKey::from_bytes(&[6; 32]);
    let key_c = SigningKey::from_bytes(&[7; 32]);
    let rename_a = RegistryEvent::sign(
        event(
            "device-a",
            1,
            None,
            "device-c",
            RegistryOperation::Rename {
                display_name: DisplayName::parse("Renamed by A").unwrap(),
            },
        ),
        &key_a,
    )
    .unwrap();
    let rename_b = RegistryEvent::sign(
        event(
            "device-b",
            1,
            None,
            "device-c",
            RegistryOperation::Rename {
                display_name: DisplayName::parse("Renamed by B").unwrap(),
            },
        ),
        &key_b,
    )
    .unwrap();
    let mut first = DeviceRegistry::default();
    let mut second = DeviceRegistry::default();
    for registry in [&mut first, &mut second] {
        registry.add_trust_anchor(trust_anchor("device-a", &key_a));
        registry.add_trust_anchor(trust_anchor("device-b", &key_b));
        registry.add_trust_anchor(trust_anchor("device-c", &key_c));
    }
    first.apply(rename_a.clone()).unwrap();
    first.apply(rename_b.clone()).unwrap();
    second.apply(rename_b).unwrap();
    second.apply(rename_a).unwrap();

    let target = DeviceId::parse("device-c").unwrap();
    assert_eq!(first.device(&target), second.device(&target));
    assert_eq!(
        first.device(&target).unwrap().display_name,
        Some(DisplayName::parse("Renamed by B").unwrap())
    );
}
