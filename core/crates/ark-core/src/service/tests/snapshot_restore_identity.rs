use super::*;

#[derive(Debug, PartialEq, Eq)]
struct RegistryIdentity {
    events: Vec<ark_domain::device_registry::RegistryEvent>,
    keys: Vec<(String, String)>,
}

async fn registry_identity(
    state: &Arc<ServiceState>,
    db_path: &std::path::Path,
) -> Result<RegistryIdentity, String> {
    let events = ark_store::test_support::DeviceRegistryFixture::open(db_path)?
        .inspect()?
        .events;
    let mut keys = Vec::new();
    for key in [
        "iroh.secret_key",
        "device_registry.local_device_id",
        "sync.peer_transport_keys",
    ] {
        let value = handle_request(
            state,
            Request::GetSyncKv {
                key: key.to_string(),
            },
        )
        .await?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("sync key {key} is missing or not a string"))?;
        keys.push((key.to_string(), value));
    }
    Ok(RegistryIdentity { events, keys })
}

#[tokio::test]
async fn db_backup_restore_preserves_live_registry_chain_and_sequence() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.db_path().to_string_lossy().to_string();
    initialize_test_database(&state, db_path.clone(),)
    .await
    .expect("initialize snapshot db");
    let db = get_db_handle(&state).expect("service database handle");
    db.set_local_sync_key_value(
        db::CommandContext::engine(),
        db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.into(),
        "restore-device".into(),
    )
    .await
    .expect("persist local registry identity");
    db.enroll_paired_device(
        db::CommandContext::engine(),
        db::EnrollPairedDeviceCommand::new(
            "restore-device".into(),
            "remote-device".into(),
            ark_net::iroh::SecretKey::generate().public().to_string(),
        ),
    )
    .await
    .expect("append initial enrollment");
    super::snapshot_restore::write_snapshot(&db_path, "ark.db.backup-registry-chain");

    handle_request(
        &state,
        Request::DeviceRegistryRename {
            target_device_id: ark_domain::device_registry::DeviceId::parse("remote-device")
                .unwrap(),
            display_name: ark_domain::device_registry::DisplayName::parse("Before restore")
                .unwrap(),
        },
    )
    .await
    .expect("append a registry event newer than the selected backup");
    let live_identity = registry_identity(&state, &fixture.db_path())
        .await
        .expect("capture live identity");

    handle_request(
        &state,
        Request::DbBackupRestore {
            backup_id: "ark.db.backup-registry-chain".into(),
        },
    )
    .await
    .expect("restore backup");
    assert_eq!(
        registry_identity(&state, &fixture.db_path())
            .await
            .expect("read restored identity"),
        live_identity
    );
    handle_request(
        &state,
        Request::DbBackupRestore {
            backup_id: "ark.db.backup-registry-chain".into(),
        },
    )
    .await
    .expect("repeat restore without changing the registry chain");
    assert_eq!(
        registry_identity(&state, &fixture.db_path())
            .await
            .expect("read identity after repeat restore"),
        live_identity
    );
    let local_head = registry_identity(&state, &fixture.db_path())
        .await
        .expect("read restored registry events")
        .events
        .iter()
        .filter(|event| event.actor_device_id().as_str() == "restore-device")
        .map(|event| event.actor_sequence() as i64)
        .max();
    assert_eq!(local_head, Some(2));

    let after_restore = handle_request(
        &state,
        Request::DeviceRegistryRename {
            target_device_id: ark_domain::device_registry::DeviceId::parse("remote-device")
                .unwrap(),
            display_name: ark_domain::device_registry::DisplayName::parse("After restore").unwrap(),
        },
    )
    .await
    .expect("append after restore");
    assert_eq!(after_restore["actor_sequence"], 3);
}
