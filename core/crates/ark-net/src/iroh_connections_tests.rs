use super::*;
use ark_domain::device_registry::{
    DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryOperation,
};

#[allow(clippy::unwrap_used)]
#[test]
fn stale_connection_cleanup_cannot_remove_replacement_generation() {
    let endpoint = iroh::SecretKey::generate().public();
    let mut connections = HashMap::from([(endpoint, (2, "replacement"))]);

    assert_eq!(take_current_connection(&mut connections, &endpoint, 1), None);
    assert_eq!(
        connections
            .get(&endpoint)
            .map(|(generation, _)| *generation),
        Some(2)
    );
    assert_eq!(
        take_current_connection(&mut connections, &endpoint, 2),
        Some("replacement")
    );
    assert!(!connections.contains_key(&endpoint));
}

#[allow(clippy::unwrap_used)]
#[test]
fn registry_event_control_must_be_addressed_before_peer_admission() {
    let registry_event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("author-device").unwrap(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("author-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: ark_domain::hlc::HLC::now("author-device").to_string(),
            operation: RegistryOperation::Rename {
                display_name: DisplayName::parse("Name").unwrap(),
            },
        },
        &ed25519_dalek::SigningKey::from_bytes(&[8; 32]),
    )
    .unwrap();
    let update = LanSyncMessage::RegistryEventUpdate {
        session_id: "session".into(),
        event: registry_event,
    };

    assert!(control_message_is_safe_without_admission(
        &LanSyncMessage::RegistrySyncStart {
            session_id: "session".into(),
        },
        false,
    ));
    assert!(control_message_is_safe_without_admission(
        &LanSyncMessage::SyncPolicy {
            revision: 1,
            digest: "policy-digest".into(),
            active: true,
            rules: Vec::new(),
        },
        false,
    ));
    assert!(control_message_is_safe_without_admission(&update, true));
    assert!(!control_message_is_safe_without_admission(&update, false));
    assert!(!control_message_is_safe_without_admission(
        &LanSyncMessage::LiveChange {
            policy_identity: None,
            change_id: "change".into(),
            entity: ark_domain::types::SyncEntity {
                id: "object".into(),
                entity_type: "note".into(),
                data: serde_json::Map::new(),
                hlc: "1:0:author-device".into(),
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            },
            origin_device_id: None,
        },
        true,
    ));
}
