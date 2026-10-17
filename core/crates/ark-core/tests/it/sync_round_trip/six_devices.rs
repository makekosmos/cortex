use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ark_application::sync_storage::{PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort};
use ark_core::sync_server::{
    IntegrationReplicationBackend, PeerIdentityBackend, SyncRuntimeStorage,
};
use ark_core::types::{PeerRecord, SyncEntity, VersionVector};
use ark_core::SqliteStorageBackend;
use tokio::sync::{watch, Notify};

use super::{make_storage_with_identity, pick_port, SyncClient, SyncServer};

#[path = "six_devices/task_merge_port.rs"]
mod task_merge_port;

#[path = "six_devices/backfill_barrier.rs"]
mod backfill_barrier;
use backfill_barrier::BackfillBarrier;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn six_devices_converge_during_initial_import_and_local_edit() {
    const DEVICE_COUNT: usize = 6;
    const INITIAL_ROWS: usize = 256;
    let fixtures = (0..DEVICE_COUNT)
        .map(|index| {
            let device_id = format!("device-{}", char::from(b'a' + index as u8));
            let (storage, endpoint) = make_storage_with_identity(&device_id);
            (device_id, storage, endpoint)
        })
        .collect::<Vec<_>>();
    for local_index in 0..DEVICE_COUNT {
        for (peer_index, (peer_id, _, peer_endpoint)) in fixtures.iter().enumerate() {
            if peer_index == local_index {
                continue;
            }
            let (local_id, local_storage, _) = &fixtures[local_index];
            local_storage
                .enroll_paired_device(local_id, peer_id, peer_endpoint)
                .await
                .expect("all devices must know their peers before concurrent sync");
        }
    }
    let source = fixtures[0].1.clone();
    source
        .apply_entity(&SyncEntity {
            entity_type: "object_type".into(),
            id: "core.task".into(),
            data: serde_json::from_value(serde_json::json!({
                "name": "Task",
                "schemaJson": "{}",
                "uiSchemaJson": "{}",
                "createdAt": "2026-09-08T00:00:00Z",
                "updatedAt": "2026-09-08T00:00:00Z",
                "systemLocked": false
            }))
            .unwrap(),
            hlc: "2026-09-08T00:00:00.000Z:000001:device-a".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        })
        .await
        .unwrap();
    for row in 0..INITIAL_ROWS {
        source
            .apply_entity(&super::selective_object_entity(
                &format!("initial-{row}"),
                "core.task",
                &format!("Initial {row}"),
                row as u64 + 1,
            ))
            .await
            .unwrap();
    }

    let profile = || {
        let mut profile = ark_domain::selective_sync::SelectiveSyncProfile::default();
        profile.set_rule(
            "type",
            "core.task",
            ark_domain::selective_sync::SyncMode::Full,
        );
        profile
    };
    source.set_selective_sync_profile(Some(profile()));

    let (release_tx, release_rx) = watch::channel(false);
    let page_requested = Arc::new(Notify::new());
    let server_storage = Arc::new(BackfillBarrier {
        inner: source.clone(),
        requested: page_requested.clone(),
        release: release_rx,
    });
    let server = Arc::new(SyncServer::new(
        server_storage as Arc<dyn SyncRuntimeStorage>,
    ));
    let port = pick_port().await;
    server
        .start_with_addr(
            "space-six-device-baseline",
            "device-a",
            Some("Device A"),
            Some(vec![format!("127.0.0.1:{port}")]),
            &format!("127.0.0.1:{port}"),
        )
        .await
        .unwrap();

    let peer = PeerRecord {
        device_id: "device-a".into(),
        device_name: "Device A".into(),
        addresses: vec![format!("127.0.0.1:{port}")],
        last_seen: chrono::Utc::now().to_rfc3339(),
        last_address: None,
        platform: None,
        app_version: None,
    };
    let page_started = page_requested.notified();
    tokio::pin!(page_started);
    let (authenticated_sender, authenticated_rx) = tokio::sync::oneshot::channel();
    let authenticated_sender = Arc::new(std::sync::Mutex::new(Some(authenticated_sender)));
    let mut devices = Vec::with_capacity(DEVICE_COUNT - 1);
    for (index, (device_id, storage, _)) in fixtures.iter().enumerate().skip(1) {
        let storage = storage.clone();
        storage.set_selective_sync_profile(Some(profile()));
        let client = Arc::new(SyncClient::new(
            storage.clone() as Arc<dyn SyncRuntimeStorage>,
            peer.clone(),
            device_id.clone(),
            format!("Device {}", char::from(b'A' + index as u8)),
            "space-six-device-baseline".into(),
            Vec::new(),
            None,
        ));
        if index == 1 {
            let authenticated_sender = authenticated_sender.clone();
            client
                .set_on_connected(Arc::new(move |_| {
                    if let Some(sender) = authenticated_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                }))
                .await;
        }
        client.start();
        devices.push((device_id.clone(), storage, client));
    }

    tokio::time::timeout(Duration::from_secs(30), page_started)
        .await
        .expect("initial sync did not request a storage page");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while server.get_connected_peer_entries().await.len() != DEVICE_COUNT - 1 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "all five devices must be authenticated before releasing backfill"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::timeout(Duration::from_secs(30), authenticated_rx)
        .await
        .expect("first device did not finish client authentication")
        .expect("client authentication callback was dropped");

    let interactive =
        super::selective_object_entity("interactive-b", "core.task", "Edited during import", 1000);
    devices[0].1.apply_entity(&interactive).await.unwrap();
    // Deliberately drop the wake-up: the connected client's periodic durable
    // feed drain must recover this committed update without waiting for reconnect.
    release_tx.send(true).unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let source_rows = source.load_entities(&HashMap::new()).await.unwrap();
        let source_ready = source_rows
            .iter()
            .any(|entity| entity.id == "interactive-b");
        let peers_ready =
            futures_util::future::join_all(devices.iter().map(|(_, storage, _)| async move {
                let rows = storage.load_entities(&HashMap::new()).await.unwrap();
                rows.iter()
                    .filter(|entity| entity.entity_type == "object")
                    .count()
                    == INITIAL_ROWS + 1
            }))
            .await
            .into_iter()
            .all(|ready| ready);
        if source_ready && peers_ready {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "six-device timeout: source_rows={}, peer_rows={:?}",
            source.load_entities(&HashMap::new()).await.unwrap().len(),
            futures_util::future::join_all(devices.iter().map(|(_, storage, _)| async move {
                storage.load_entities(&HashMap::new()).await.unwrap().len()
            }))
            .await
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let rows = devices[4].1.load_entities(&HashMap::new()).await.unwrap();
    assert!(rows.iter().any(|entity| entity.id == "initial-255"));
    assert!(rows.iter().any(|entity| entity.id == "interactive-b"));
    let mut narrowed = profile();
    narrowed.set_rule(
        "type",
        "core.task",
        ark_domain::selective_sync::SyncMode::None,
    );
    devices[4].1.set_selective_sync_profile(Some(narrowed));
    let projected = devices[4].1.load_entities(&HashMap::new()).await.unwrap();
    assert_eq!(
        projected
            .iter()
            .filter(|entity| entity.entity_type == "object")
            .count(),
        0,
        "narrowed profile must exclude task objects from outgoing projection"
    );
    devices[4].1.set_selective_sync_profile(None);
    let persisted = devices[4].1.load_entities(&HashMap::new()).await.unwrap();
    let persisted_objects: Vec<_> = persisted
        .iter()
        .filter(|entity| entity.entity_type == "object")
        .collect();
    assert_eq!(persisted_objects.len(), INITIAL_ROWS + 1);
    assert!(
        persisted_objects
            .iter()
            .any(|entity| entity.id == "interactive-b"),
        "interactive row must remain persisted after profile narrowing"
    );
    for (_, _, client) in devices {
        client.stop();
    }
    server.stop().await;
}
