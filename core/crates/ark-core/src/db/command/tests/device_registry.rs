use super::*;

#[path = "device_registry/atomicity.rs"]
mod atomicity;
#[path = "device_registry/commands.rs"]
mod commands;
#[path = "device_registry/handshake.rs"]
mod handshake;

#[tokio::test]
async fn paired_device_enrollment_is_atomic_idempotent_and_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();

    let first = handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new(
                "local-device".into(),
                "paired-device".into(),
                endpoint.clone(),
            ),
        )
        .await
        .unwrap();
    let DeviceEnrollmentOutcome::Applied {
        event_id,
        event_hash,
    } = first
    else {
        panic!("first enrollment must append a signed event");
    };
    assert_eq!(event_id, "local-device:1");
    assert_eq!(event_hash.len(), 64);
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
    drop(handle);

    let (handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();
    let retry = handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new(
                "local-device".into(),
                "paired-device".into(),
                endpoint.clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        retry,
        DeviceEnrollmentOutcome::AlreadyApplied {
            event_id: Some(event_id),
        }
    );

    let state =
        ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(&db_path))
            .unwrap()
            .inspect()
            .unwrap();
    assert_eq!(state.events.len(), 1);
    assert_eq!(
        state.identities["paired-device"].signing_public_key,
        endpoint
            .parse::<ark_net::iroh::EndpointId>()
            .unwrap()
            .as_bytes()
    );
    assert_eq!(
        state.transport_key_pins.get("paired-device"),
        Some(&endpoint)
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn paired_device_enrollment_rolls_back_event_projection_and_pin_together() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-rollback.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();
    ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(&db_path))
        .unwrap()
        .reject_paired_device_projection()
        .unwrap();

    let result = handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await;
    assert!(matches!(result, Err(DbError::Storage(_))));
    let state = ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(
        &directory.path().join("device-registry-rollback.db"),
    ))
    .unwrap()
    .inspect()
    .unwrap();
    assert!(state.events.is_empty(), "partial registry event remained");
    assert!(
        !state.identities.contains_key("paired-device"),
        "partial device projection remained"
    );
    assert!(
        !state.transport_key_pins.contains_key("paired-device"),
        "partial transport-key pin remained"
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn registry_history_is_paged_and_remote_events_apply_idempotently() {
    use ark_domain::device_registry::{
        DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };
    use ed25519_dalek::SigningKey;

    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-exchange.db")
        .to_string_lossy()
        .into_owned();
    let remote_key = ark_net::iroh::SecretKey::generate();
    let endpoint = remote_key.public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await
        .unwrap();

    let (high_watermark, total_events, first_page) = handle
        .query_with_store(
            CommandContext::sync_system(CommandClass::SyncLive),
            LoadRegistryEventsQuery::new(0, None, 1),
        )
        .await
        .unwrap();
    assert_eq!(high_watermark, 1);
    assert_eq!(total_events, 1);
    assert_eq!(first_page.len(), 1);
    let (same_watermark, same_total, second_page) = handle
        .query_with_store(
            CommandContext::sync_system(CommandClass::SyncLive),
            LoadRegistryEventsQuery::new(1, Some(high_watermark), 64),
        )
        .await
        .unwrap();
    assert_eq!(same_watermark, 1);
    assert_eq!(same_total, 1);
    assert!(second_page.is_empty());

    let signing_key = SigningKey::from_bytes(&remote_key.to_bytes());
    let rename = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("paired-device").unwrap(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("paired-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("paired-device").to_string(),
            operation: RegistryOperation::Rename {
                display_name: DisplayName::parse("Pocket device").unwrap(),
            },
        },
        &signing_key,
    )
    .unwrap();
    let apply = || ApplyRegistryEventsCommand::new("paired-device".into(), vec![rename.clone()]);
    let first_digest = handle
        .apply_registry_events(
            CommandContext::sync_peer("paired-device".into(), CommandClass::SyncLive),
            apply(),
        )
        .await
        .unwrap();
    let retry_digest = handle
        .apply_registry_events(
            CommandContext::sync_peer("paired-device".into(), CommandClass::SyncLive),
            apply(),
        )
        .await
        .unwrap();
    assert_eq!(first_digest.digest, retry_digest.digest);
    assert_eq!(
        first_digest.applied_events.as_slice(),
        std::slice::from_ref(&rename)
    );
    assert!(retry_digest.applied_events.is_empty());
    let state = ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(
        &directory.path().join("device-registry-exchange.db"),
    ))
    .unwrap()
    .inspect()
    .unwrap();
    assert_eq!(
        state
            .entries
            .iter()
            .find(|device| device.device_id == "paired-device")
            .unwrap()
            .display_name
            .as_deref(),
        Some("Pocket device")
    );
    assert_eq!(
        state
            .events
            .iter()
            .filter(|event| event.actor_device_id().as_str() == "paired-device")
            .count(),
        1
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn disabled_peer_cannot_commit_an_inbound_entity_after_session_admission() {
    use ark_domain::device_registry::DeviceId;

    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-revocation.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await
        .unwrap();

    let admitted_context = CommandContext::authenticated_peer("paired-device".into());
    let disabled = handle
        .append_local_registry_event(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new(
                "local-device".into(),
                ark_domain::device_registry::RegistryCommand::Disable {
                    target_device_id: DeviceId::parse("paired-device").unwrap(),
                },
            ),
        )
        .await
        .unwrap()
        .expect("local disable must append a signed registry event");
    assert_eq!(disabled.event_id(), "local-device:2");

    let write = ark_application::sync_storage::InboxReceiptWrite::new(
        "paired-device".into(),
        "revoked-receipt".into(),
        "{}".into(),
    );
    let receipt = handle.receive_inbox_receipt(admitted_context, write).await;
    assert!(matches!(
        receipt,
        Err(DbError::Permission(reason)) if reason == "sync_peer_not_admitted"
    ));

    let entity = crate::types::SyncEntity {
        entity_type: "note".into(),
        id: "remote-note".into(),
        data: serde_json::Map::new(),
        hlc: crate::hlc::HLC::now("paired-device").to_string(),
        deleted: None,
        origin_device_id: Some("paired-device".into()),
        origin_seq: None,
    };
    let apply = handle
        .submit_authenticated_inbound_entity(
            CommandContext::authenticated_peer("paired-device".into()),
            ark_application::sync_storage::InboxReceiptWrite::new(
                "paired-device".into(),
                "revoked-session-write".into(),
                serde_json::to_string(&entity).unwrap(),
            ),
            entity,
        )
        .await;
    assert!(matches!(
        apply,
        Err(DbError::Permission(reason)) if reason == "sync_peer_not_admitted"
    ));
    let state = ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(
        &directory.path().join("device-registry-revocation.db"),
    ))
    .unwrap();
    assert!(!state.object_exists("remote-note").unwrap());
    assert!(!state.has_inbox_receipt("revoked-session-write").unwrap());
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn clear_all_removes_registry_before_iroh_identity_is_cleared() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-clear.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await
        .unwrap();

    handle
        .submit_clear_all(CommandContext::engine_maintenance())
        .await
        .unwrap();
    let state = ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(
        &directory.path().join("device-registry-clear.db"),
    ))
    .unwrap()
    .inspect()
    .unwrap();
    assert!(state.events.is_empty(), "clear_all left registry events");
    assert!(
        state.entries.is_empty(),
        "clear_all left registry projections"
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
