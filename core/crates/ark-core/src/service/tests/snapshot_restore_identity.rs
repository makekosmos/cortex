use super::*;

type RegistryIdentity = (
    Vec<(i64, i64, String, String, String)>,
    Vec<(String, String)>,
);

fn registry_identity(state: &Arc<ServiceState>) -> Result<RegistryIdentity, String> {
    with_conn(state, |connection| {
        let events = connection
            .prepare(
                "SELECT registry_position, actor_sequence, event_id, event_hash, event_json
                 FROM device_registry_events ORDER BY registry_position",
            )
            .map_err(|error| error.to_string())?
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let keys = connection
            .prepare(
                "SELECT key, value FROM sync_kv
                 WHERE key IN ('iroh.secret_key', 'device_registry.local_device_id',
                               'sync.peer_transport_keys') ORDER BY key",
            )
            .map_err(|error| error.to_string())?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        Ok((events, keys))
    })
}

#[tokio::test]
async fn db_backup_restore_preserves_live_registry_chain_and_sequence() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.db_path().to_string_lossy().to_string();
    handle_request(
        &state,
        Request::Init {
            db_path: db_path.clone(),
        },
    )
    .await
    .expect("initialize snapshot db");
    let db = get_db_handle(&state).expect("service database handle");
    db.submit(
        db::CommandContext::engine(),
        db::SetLocalSyncKeyValueCommand::new(
            db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.into(),
            "restore-device".into(),
        ),
    )
    .await
    .expect("persist local registry identity");
    db.submit(
        db::CommandContext::engine(),
        db::EnrollPairedDeviceCommand::new(
            "restore-device".into(),
            "remote-device".into(),
            iroh::SecretKey::generate().public().to_string(),
        ),
    )
    .await
    .expect("append initial enrollment");
    super::snapshot_restore::write_snapshot(&state, &db_path, "ark.db.backup-registry-chain");

    handle_request(
        &state,
        Request::DeviceRegistryRename {
            target_device_id: crate::domain::device_registry::DeviceId::parse("remote-device")
                .unwrap(),
            display_name: crate::domain::device_registry::DisplayName::parse("Before restore")
                .unwrap(),
        },
    )
    .await
    .expect("append a registry event newer than the selected backup");
    let live_identity = registry_identity(&state).expect("capture live identity");

    handle_request(
        &state,
        Request::DbBackupRestore {
            backup_id: "ark.db.backup-registry-chain".into(),
        },
    )
    .await
    .expect("restore backup");
    assert_eq!(
        registry_identity(&state).expect("read restored identity"),
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
        registry_identity(&state).expect("read identity after repeat restore"),
        live_identity
    );
    let local_head = with_conn(&state, |connection| {
        connection
            .query_row(
                "SELECT MAX(actor_sequence) FROM device_registry_events
                 WHERE actor_device_id = 'restore-device'",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(|error| error.to_string())
    })
    .expect("read restored local registry head");
    assert_eq!(local_head, Some(2));

    let after_restore = handle_request(
        &state,
        Request::DeviceRegistryRename {
            target_device_id: crate::domain::device_registry::DeviceId::parse("remote-device")
                .unwrap(),
            display_name: crate::domain::device_registry::DisplayName::parse("After restore")
                .unwrap(),
        },
    )
    .await
    .expect("append after restore");
    assert_eq!(after_restore["actor_sequence"], 3);
}
