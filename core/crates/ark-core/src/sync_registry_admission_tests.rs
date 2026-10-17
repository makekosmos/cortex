use super::*;
use crate::domain::device_registry::{
    DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryOperation,
};
use crate::replication_storage::PeerIdentityBackend;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn rejects_page_after_declared_count_without_applying_its_event() {
    let connection = rusqlite::Connection::open_in_memory().expect("open test database");
    crate::db::init_schema(&connection).expect("initialize test schema");
    let backend = Arc::new(crate::db::SqliteStorageBackend::for_test(Arc::new(
        Mutex::new(connection),
    )));
    backend
        .set_device_id("local-device")
        .expect("set local device id");
    let remote_key = ark_net::iroh::SecretKey::generate();
    backend
        .enroll_paired_device(
            "local-device",
            "remote-device",
            &remote_key.public().to_string(),
        )
        .await
        .expect("enroll the remote event author");

    let remote_signing_key = ed25519_dalek::SigningKey::from_bytes(&remote_key.to_bytes());
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("remote-device").expect("valid actor"),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("remote-device").expect("valid target"),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("remote-device").to_string(),
            operation: RegistryOperation::Rename {
                display_name: DisplayName::parse("Renamed after final page").expect("valid name"),
            },
        },
        &remote_signing_key,
    )
    .expect("sign a valid registry update");

    let mut admission = DirectRegistryAdmission {
        local_device_id: DeviceId::parse("local-device").expect("valid local id"),
        peer_device_id: DeviceId::parse("remote-device").expect("valid peer id"),
        space_id: "space-test".into(),
        protocol: NegotiatedProtocol {
            version: 1,
            capabilities: BTreeSet::from([ProtocolCapability::RegistryHistoryV1]),
        },
        session_id: Some("session-test".into()),
        session_revoked: false,
        phase: AdmissionPhase::Exchanging {
            incoming_page_count: Some(1),
            next_page_index: 1,
            local_digest: None,
            remote_digest: None,
            start_received: true,
        },
    };

    let result = admission
        .handle(
            backend.as_ref(),
            LanSyncMessage::RegistryEventsPage {
                session_id: "session-test".into(),
                page_index: 1,
                page_count: 1,
                events: vec![event],
            },
        )
        .await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("page index must stay below its declared count"),
    };
    assert_eq!(error, "registry_sync_page_bounds_or_order");

    let (_, total_events, events) = backend
        .load_device_registry_events_page(0, None, 32)
        .await
        .expect("read registry after rejected page");
    assert_eq!(total_events, 1);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].actor_device_id().as_str(), "local-device");
}

#[tokio::test]
async fn does_not_admit_local_device_disabled_by_registry_page() {
    let connection = rusqlite::Connection::open_in_memory().expect("open test database");
    crate::db::init_schema(&connection).expect("initialize test schema");
    let backend = Arc::new(crate::db::SqliteStorageBackend::for_test(Arc::new(
        Mutex::new(connection),
    )));
    backend
        .set_device_id("local-device")
        .expect("set local device id");
    let remote_key = ark_net::iroh::SecretKey::generate();
    backend
        .enroll_paired_device(
            "local-device",
            "remote-device",
            &remote_key.public().to_string(),
        )
        .await
        .expect("enroll the remote event author");

    let remote_signing_key = ed25519_dalek::SigningKey::from_bytes(&remote_key.to_bytes());
    let disable_local = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("remote-device").expect("valid actor"),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("local-device").expect("valid target"),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("remote-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &remote_signing_key,
    )
    .expect("sign local-device disable event");
    let mut admission = DirectRegistryAdmission {
        local_device_id: DeviceId::parse("local-device").expect("valid local id"),
        peer_device_id: DeviceId::parse("remote-device").expect("valid peer id"),
        space_id: "space-test".into(),
        protocol: NegotiatedProtocol {
            version: 1,
            capabilities: BTreeSet::from([ProtocolCapability::RegistryHistoryV1]),
        },
        session_id: Some("session-test".into()),
        session_revoked: false,
        phase: AdmissionPhase::Exchanging {
            incoming_page_count: None,
            next_page_index: 0,
            local_digest: None,
            remote_digest: None,
            start_received: true,
        },
    };

    let result = admission
        .handle(
            backend.as_ref(),
            LanSyncMessage::RegistryEventsPage {
                session_id: "session-test".into(),
                page_index: 0,
                page_count: 1,
                events: vec![disable_local],
            },
        )
        .await;
    let update = result.expect("signed disable event should be committed for relay");
    assert!(update.close_session);
    assert!(!update.became_ready);
    assert_eq!(update.applied_events.len(), 1);
}
