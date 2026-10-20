use super::*;
use rusqlite::OptionalExtension;

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
        .submit(
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
        .submit(
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

    handle.with_connection(|connection| {
        let event_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM device_registry_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        let target_key: Vec<u8> = connection
            .query_row(
                "SELECT signing_public_key
                 FROM device_registry_projection
                 WHERE device_id = 'paired-device'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let pinned: String = connection
            .query_row(
                "SELECT value FROM sync_kv WHERE key = 'sync.peer_transport_keys'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 1);
        assert_eq!(
            target_key,
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
    });
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
        connection
            .execute_batch(
                "CREATE TRIGGER reject_device_registry_projection
                 BEFORE INSERT ON device_registry_projection
                 WHEN NEW.device_id = 'paired-device'
                 BEGIN SELECT RAISE(ABORT, 'injected projection failure'); END;",
            )
            .unwrap();
    });

    let result = handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await;
    assert!(matches!(result, Err(DbError::Storage(_))));
    handle.with_connection(|connection| {
        for table in ["device_registry_events", "device_registry_projection"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "partial write remained in {table}");
        }
        let pin: Option<String> = connection
            .query_row(
                "SELECT value FROM sync_kv WHERE key = 'sync.peer_transport_keys'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(pin, None);
    });
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
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await
        .unwrap();

    let (high_watermark, total_events, first_page) = handle
        .query(
            CommandContext::sync_system(CommandClass::SyncLive),
            LoadRegistryEventsQuery::new(0, None, 1),
        )
        .await
        .unwrap();
    assert_eq!(high_watermark, 1);
    assert_eq!(total_events, 1);
    assert_eq!(first_page.len(), 1);
    let (same_watermark, same_total, second_page) = handle
        .query(
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
        .submit(
            CommandContext::sync_peer("paired-device".into(), CommandClass::SyncLive),
            apply(),
        )
        .await
        .unwrap();
    let retry_digest = handle
        .submit(
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
        let alias: String = connection
            .query_row(
                "SELECT display_name FROM device_registry_projection
                 WHERE device_id = 'paired-device'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(alias, "Pocket device");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM device_registry_events
                 WHERE actor_device_id = 'paired-device'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
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
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await
        .unwrap();

    let (secret, previous_hash) = handle.with_connection(|connection| {
        let secret = crate::iroh_transport::load_or_generate_secret_key(connection).unwrap();
        let previous_hash = connection
            .query_row(
                "SELECT event_hash FROM device_registry_events
                 WHERE actor_device_id = 'local-device' ORDER BY actor_sequence DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
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
        .submit(
            CommandContext::sync_peer("paired-device".into(), CommandClass::SyncLive),
            ApplyRegistryEventsCommand::new("paired-device".into(), vec![disable]),
        )
        .await
        .unwrap();
    assert_eq!(applied.disabled_device_ids, ["paired-device"]);

    let receipt = handle
        .submit(
            CommandContext::authenticated_peer("paired-device".into()),
            ReceiveInboxReceiptCommand::new(
                "paired-device".into(),
                "revoked-receipt".into(),
                "{}".into(),
            ),
        )
        .await;
    assert!(matches!(
        receipt,
        Err(DbError::Permission(reason)) if reason == "sync_peer_not_admitted"
    ));

    let apply = handle
        .submit(
            CommandContext::authenticated_peer("paired-device".into()),
            ApplyAuthenticatedSyncEntityCommand::with_message_id(
                crate::types::SyncEntity {
                    entity_type: "note".into(),
                    id: "remote-note".into(),
                    data: serde_json::Map::new(),
                    hlc: crate::hlc::HLC::now("paired-device").to_string(),
                    deleted: None,
                    origin_device_id: Some("paired-device".into()),
                    origin_seq: None,
                },
                "revoked-session-write".into(),
            ),
        )
        .await;
    assert!(matches!(
        apply,
        Err(DbError::Permission(reason)) if reason == "sync_peer_not_admitted"
    ));
    handle.with_connection(|connection| {
        let object_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM objects WHERE id = 'remote-note'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let receipt_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_sync_inbox WHERE message_id = 'revoked-session-write'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(object_count, 0);
        assert_eq!(receipt_count, 0);
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
        .submit(
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
        for table in ["device_registry_events", "device_registry_projection"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "clear_all left {count} rows in {table}");
        }
    });
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
