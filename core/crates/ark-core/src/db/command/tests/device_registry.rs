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

    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
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

    let (event_count, target_key, pinned) = handle.with_connection(|connection| {
        let (_, event_count, _) = ark_store::device_registry::load_events_page(
            connection,
            0,
            None,
            ark_store::device_registry::MAX_REGISTRY_EVENTS_PAGE_SIZE,
        )
        .unwrap();
        let identity =
            ark_store::device_registry::load_identity(connection, "paired-device").unwrap();
        let pinned = ark_store::sync_kv::get_sync_kv(
            connection,
            ark_store::device_registry::PEER_TRANSPORT_KEYS_KEY,
        )
        .unwrap()
        .unwrap();
        (event_count, identity, pinned)
    });
    assert_eq!(event_count, 1);
    assert_eq!(
        target_key.unwrap().signing_public_key,
        endpoint
            .parse::<ark_net::iroh::EndpointId>()
            .unwrap()
            .as_bytes()
    );
    assert_eq!(
        serde_json::from_str::<std::collections::HashMap<String, String>>(&pinned)
            .unwrap()
            .get("paired-device"),
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
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle.with_connection(|connection| {
        ark_store::test_support::reject_paired_device_projection(connection).unwrap();
    });

    let result = handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await;
    assert!(matches!(result, Err(DbError::Storage(_))));
    let (event_count, target, pin) = handle.with_connection(|connection| {
        let (_, event_count, _) = ark_store::device_registry::load_events_page(
            connection,
            0,
            None,
            ark_store::device_registry::MAX_REGISTRY_EVENTS_PAGE_SIZE,
        )
        .unwrap();
        let target =
            ark_store::device_registry::load_identity(connection, "paired-device").unwrap();
        let pin = ark_store::sync_kv::get_sync_kv(
            connection,
            ark_store::device_registry::PEER_TRANSPORT_KEYS_KEY,
        )
        .unwrap();
        (event_count, target, pin)
    });
    assert_eq!(event_count, 0, "partial registry event remained");
    assert_eq!(target, None, "partial device projection remained");
    assert_eq!(pin, None, "partial transport-key pin remained");
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
    handle.with_connection(|connection| {
        let paired_device = ark_store::device_registry::load_entries(connection)
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "paired-device")
            .unwrap();
        assert_eq!(paired_device.display_name.as_deref(), Some("Pocket device"));
        let (_, _, events) = ark_store::device_registry::load_events_page(
            connection,
            0,
            None,
            ark_store::device_registry::MAX_REGISTRY_EVENTS_PAGE_SIZE,
        )
        .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.actor_device_id().as_str() == "paired-device")
                .count(),
            1
        );
    });
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn disabled_peer_cannot_commit_an_inbound_entity_after_session_admission() {
    use ark_domain::device_registry::{
        DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };
    use ed25519_dalek::SigningKey;

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

    let (secret, previous_hash) = handle.with_connection(|connection| {
        let secret = crate::iroh_transport::load_or_generate_secret_key(connection).unwrap();
        let (_, previous_hash) =
            ark_store::device_registry::load_actor_head(connection, "local-device")
                .unwrap()
                .unwrap();
        (secret, previous_hash)
    });
    let local_key = SigningKey::from_bytes(&secret.to_bytes());
    let disable = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("local-device").unwrap(),
            actor_sequence: 2,
            previous_event_hash: Some(previous_hash),
            target_device_id: DeviceId::parse("paired-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("local-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &local_key,
    )
    .unwrap();
    let applied = handle
        .apply_registry_events(
            CommandContext::sync_peer("paired-device".into(), CommandClass::SyncLive),
            ApplyRegistryEventsCommand::new("paired-device".into(), vec![disable]),
        )
        .await
        .unwrap();
    assert_eq!(applied.disabled_device_ids, ["paired-device"]);

    let context = CommandContext::authenticated_peer("paired-device".into());
    let write = ark_application::sync_storage::InboxReceiptWrite::new(
        "paired-device".into(),
        "revoked-receipt".into(),
        "{}".into(),
    );
    let receipt = handle.receive_inbox_receipt(context, write).await;
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
    handle.with_connection(|connection| {
        let object = ark_store::objects::get_object(connection, "remote-note").unwrap();
        let receipt =
            ark_store::test_support::has_inbox_receipt(connection, "revoked-session-write")
                .unwrap();
        assert_eq!(object, None);
        assert!(!receipt);
    });
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
    handle.with_connection(|connection| {
        let (_, event_count, _) = ark_store::device_registry::load_events_page(
            connection,
            0,
            None,
            ark_store::device_registry::MAX_REGISTRY_EVENTS_PAGE_SIZE,
        )
        .unwrap();
        assert_eq!(event_count, 0, "clear_all left registry events");
        assert!(
            ark_store::device_registry::load_entries(connection)
                .unwrap()
                .is_empty(),
            "clear_all left registry projections"
        );
    });
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
