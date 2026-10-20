use super::*;

#[tokio::test]
async fn registry_event_batch_rolls_back_prior_events_after_sequence_conflict() {
    use ark_domain::device_registry::{
        DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };
    use ed25519_dalek::SigningKey;

    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-batch-rollback.db")
        .to_string_lossy()
        .into_owned();
    let endpoint_key = ark_net::iroh::SecretKey::generate();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new(
                "local-device".into(),
                "paired-device".into(),
                endpoint_key.public().to_string(),
            ),
        )
        .await
        .unwrap();

    let signing_key = SigningKey::from_bytes(&endpoint_key.to_bytes());
    let event = |name: &str| {
        RegistryEvent::sign(
            RegistryEventDraft {
                actor_device_id: DeviceId::parse("paired-device").unwrap(),
                actor_sequence: 1,
                previous_event_hash: None,
                target_device_id: DeviceId::parse("paired-device").unwrap(),
                actor_key_epoch: 1,
                actor_admission_epoch: 1,
                hlc: crate::hlc::HLC::now("paired-device").to_string(),
                operation: RegistryOperation::Rename {
                    display_name: DisplayName::parse(name).unwrap(),
                },
            },
            &signing_key,
        )
        .unwrap()
    };
    let first = event("First name");
    let conflicting = event("Conflicting name");
    let result = handle
        .apply_registry_events(
            CommandContext::sync_peer("paired-device".into(), CommandClass::SyncLive),
            ApplyRegistryEventsCommand::new("paired-device".into(), vec![first, conflicting]),
        )
        .await;
    assert!(matches!(
        result,
        Err(DbError::Conflict(reason)) if reason == "device_registry_sequence_conflict"
    ));
    handle.with_connection(|connection| {
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
            0
        );
        let paired_device = ark_store::device_registry::load_entries(connection)
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "paired-device")
            .unwrap();
        assert_eq!(paired_device.display_name, None);
    });
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
