use super::*;
use crate::domain::device_registry::{DeviceId, DisplayName, RegistryCommand, RegistryOperation};

#[tokio::test]
async fn local_registry_changes_are_signed_atomic_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-local-command.db")
        .to_string_lossy()
        .into_owned();
    let remote_key = iroh::SecretKey::generate();
    let (handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new(
                "local-device".into(),
                "remote-device".into(),
                remote_key.public().to_string(),
            ),
        )
        .await
        .unwrap();

    let rename = RegistryCommand::Rename {
        target_device_id: DeviceId::parse("remote-device").unwrap(),
        display_name: DisplayName::parse("Office laptop").unwrap(),
    };
    let rename_event = handle
        .submit(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new("local-device".into(), rename.clone()),
        )
        .await
        .unwrap()
        .expect("first rename must append a signed event");
    assert_eq!(rename_event.event_id(), "local-device:2");
    assert!(matches!(
        rename_event.operation(),
        RegistryOperation::Rename { display_name }
            if display_name.as_str() == "Office laptop"
    ));
    assert!(handle
        .submit(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new("local-device".into(), rename),
        )
        .await
        .unwrap()
        .is_none());

    let disable = RegistryCommand::Disable {
        target_device_id: DeviceId::parse("remote-device").unwrap(),
    };
    let disable_event = handle
        .submit(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new("local-device".into(), disable.clone()),
        )
        .await
        .unwrap()
        .expect("first disable must append a signed event");
    assert_eq!(disable_event.event_id(), "local-device:3");
    assert!(handle
        .submit(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new("local-device".into(), disable),
        )
        .await
        .unwrap()
        .is_none());

    handle.with_connection(|connection| {
        let (name, disabled_event_id, event_count): (Option<String>, Option<String>, i64) =
            connection
                .query_row(
                    "SELECT projection.display_name, projection.disabled_event_id,
                            (SELECT COUNT(*) FROM device_registry_events)
                     FROM device_registry_projection AS projection
                     WHERE projection.device_id = 'remote-device'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
        assert_eq!(name.as_deref(), Some("Office laptop"));
        assert_eq!(disabled_event_id.as_deref(), Some("local-device:3"));
        assert_eq!(event_count, 3);
    });

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
    drop(handle);
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle.with_connection(|connection| {
        let (name, disabled): (Option<String>, Option<String>) = connection
            .query_row(
                "SELECT display_name, disabled_event_id FROM device_registry_projection
                 WHERE device_id = 'remote-device'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name.as_deref(), Some("Office laptop"));
        assert_eq!(disabled.as_deref(), Some("local-device:3"));
    });
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn outbound_sync_admission_closes_after_device_disable() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-outbound-admission.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "remote-device".into(), endpoint),
        )
        .await
        .unwrap();

    let query = crate::db::RegistryPeerAdmissionQuery::new("remote-device".into());
    assert!(handle
        .query(
            CommandContext::sync_peer("remote-device".into(), CommandClass::SyncLive),
            query.clone(),
        )
        .await
        .unwrap());

    handle
        .submit(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new(
                "local-device".into(),
                RegistryCommand::Disable {
                    target_device_id: DeviceId::parse("remote-device").unwrap(),
                },
            ),
        )
        .await
        .unwrap();
    assert!(!handle
        .query(
            CommandContext::sync_peer("remote-device".into(), CommandClass::SyncLive),
            query,
        )
        .await
        .unwrap());

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
