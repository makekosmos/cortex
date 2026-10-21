use super::*;
use ark_domain::device_registry::{DeviceId, DisplayName, RegistryCommand, RegistryOperation};

#[tokio::test]
async fn local_registry_event_and_projection_roll_back_together() {
    let directory = crate::test_support::temp_dir();
    let db_path = directory
        .path()
        .join("device-registry-local-rollback.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "remote-device".into(), endpoint),
        )
        .await
        .unwrap();
    ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(&db_path))
        .unwrap()
        .reject_remote_device_rename()
        .unwrap();

    let result = handle
        .append_local_registry_event(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new(
                "local-device".into(),
                RegistryCommand::Rename {
                    target_device_id: DeviceId::parse("remote-device").unwrap(),
                    display_name: DisplayName::parse("Office laptop").unwrap(),
                },
            ),
        )
        .await;
    assert!(matches!(result, Err(DbError::Storage(_))));
    let state = ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(
        &directory.path().join("device-registry-local-rollback.db"),
    ))
    .unwrap()
    .inspect()
    .unwrap();
    assert_eq!(state.events.len(), 1);
    assert_eq!(
        state
            .entries
            .iter()
            .find(|device| device.device_id == "remote-device")
            .unwrap()
            .display_name,
        None
    );

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn local_registry_changes_are_signed_atomic_and_idempotent() {
    let directory = crate::test_support::temp_dir();
    let db_path = directory
        .path()
        .join("device-registry-local-command.db")
        .to_string_lossy()
        .into_owned();
    let remote_key = ark_net::iroh::SecretKey::generate();
    let (handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
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
        .append_local_registry_event(
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
        .append_local_registry_event(
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
        .append_local_registry_event(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new("local-device".into(), disable.clone()),
        )
        .await
        .unwrap()
        .expect("first disable must append a signed event");
    assert_eq!(disable_event.event_id(), "local-device:3");
    assert!(handle
        .append_local_registry_event(
            CommandContext::engine(),
            AppendLocalRegistryEventCommand::new("local-device".into(), disable),
        )
        .await
        .unwrap()
        .is_none());

    let state =
        ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(&db_path))
            .unwrap()
            .inspect()
            .unwrap();
    let entry = state
        .entries
        .iter()
        .find(|device| device.device_id == "remote-device")
        .unwrap();
    assert_eq!(entry.display_name.as_deref(), Some("Office laptop"));
    assert_eq!(
        state.identities["remote-device"]
            .disabled_event_id
            .as_deref(),
        Some("local-device:3")
    );
    assert_eq!(state.events.len(), 3);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
    drop(handle);
    let (_handle, mut owner) = DbHandle::open(db_path.clone(), "local-device".into())
        .await
        .unwrap();
    let state =
        ark_store::test_support::DeviceRegistryFixture::open(std::path::Path::new(&db_path))
            .unwrap()
            .inspect()
            .unwrap();
    let entry = state
        .entries
        .iter()
        .find(|device| device.device_id == "remote-device")
        .unwrap();
    assert_eq!(entry.display_name.as_deref(), Some("Office laptop"));
    assert_eq!(
        state.identities["remote-device"]
            .disabled_event_id
            .as_deref(),
        Some("local-device:3")
    );
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn outbound_sync_admission_closes_after_device_disable() {
    let directory = crate::test_support::temp_dir();
    let db_path = directory
        .path()
        .join("device-registry-outbound-admission.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "remote-device".into(), endpoint),
        )
        .await
        .unwrap();

    let query = crate::db::RegistryPeerAdmissionQuery::new("remote-device".into());
    assert!(handle
        .query_with_store(
            CommandContext::sync_peer("remote-device".into(), CommandClass::SyncLive),
            query.clone(),
        )
        .await
        .unwrap());

    handle
        .append_local_registry_event(
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
        .query_with_store(
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
