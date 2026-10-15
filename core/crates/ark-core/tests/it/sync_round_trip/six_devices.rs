use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ark_core::data_platform;
use ark_core::sync_server::StorageBackend;
use ark_core::types::{PeerRecord, SyncEntity, VersionVector};
use ark_core::SqliteStorageBackend;
use tokio::sync::{watch, Notify};

use super::{make_storage, pick_port, SyncClient, SyncServer};

struct BackfillBarrier {
    inner: Arc<SqliteStorageBackend>,
    requested: Arc<Notify>,
    release: watch::Receiver<bool>,
}

#[async_trait::async_trait]
impl StorageBackend for BackfillBarrier {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity> {
        self.inner.load_entities(vector).await
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        self.requested.notify_one();
        let mut release = self.release.clone();
        while !*release.borrow() {
            if release.changed().await.is_err() {
                return Vec::new();
            }
        }
        self.inner.load_entities_page(vector, offset, limit).await
    }

    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        self.inner.apply_entity(entity).await
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        self.inner.get_kv(key).await
    }

    async fn set_kv(&self, key: &str, value: &str) {
        self.inner.set_kv(key, value).await
    }

    async fn usage_complete_through(&self) -> HashMap<String, u64> {
        self.inner.usage_complete_through().await
    }

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        self.inner.filter_outgoing_entity(entity)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn six_devices_converge_during_initial_import_and_local_edit() {
    const DEVICE_COUNT: usize = 6;
    const INITIAL_ROWS: usize = 256;
    let source = make_storage("device-a");
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
        let mut profile = data_platform::SelectiveSyncProfile::default();
        profile.set_rule("type", "core.task", data_platform::SyncMode::Full);
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
    let server = Arc::new(SyncServer::new(server_storage as Arc<dyn StorageBackend>));
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
    for index in 1..DEVICE_COUNT {
        let device_id = format!("device-{}", char::from(b'a' + index as u8));
        let storage = make_storage(&device_id);
        storage.set_selective_sync_profile(Some(profile()));
        let client = Arc::new(SyncClient::new(
            storage.clone() as Arc<dyn StorageBackend>,
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
        devices.push((device_id, storage, client));
    }

    tokio::time::timeout(Duration::from_secs(10), page_started)
        .await
        .expect("initial sync did not request a storage page");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while server.get_connected_peer_entries().await.len() != DEVICE_COUNT - 1 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "all five devices must be authenticated before releasing backfill"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::timeout(Duration::from_secs(10), authenticated_rx)
        .await
        .expect("first device did not finish client authentication")
        .expect("client authentication callback was dropped");

    let interactive =
        super::selective_object_entity("interactive-b", "core.task", "Edited during import", 1000);
    devices[0].1.apply_entity(&interactive).await.unwrap();
    devices[0].2.broadcast_live_change(interactive).await;
    release_tx.send(true).unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let source_rows = source.load_entities(&HashMap::new()).await;
        let source_ready = source_rows
            .iter()
            .any(|entity| entity.id == "interactive-b");
        let peers_ready =
            futures_util::future::join_all(devices.iter().map(|(_, storage, _)| async move {
                let rows = storage.load_entities(&HashMap::new()).await;
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
            source.load_entities(&HashMap::new()).await.len(),
            futures_util::future::join_all(devices.iter().map(|(_, storage, _)| async move {
                storage.load_entities(&HashMap::new()).await.len()
            }))
            .await
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let rows = devices[4].1.load_entities(&HashMap::new()).await;
    assert!(rows.iter().any(|entity| entity.id == "initial-255"));
    assert!(rows.iter().any(|entity| entity.id == "interactive-b"));
    let mut narrowed = profile();
    narrowed.set_rule("type", "core.task", data_platform::SyncMode::None);
    devices[4].1.set_selective_sync_profile(Some(narrowed));
    let projected = devices[4].1.load_entities(&HashMap::new()).await;
    assert_eq!(
        projected
            .iter()
            .filter(|entity| entity.entity_type == "object")
            .count(),
        0,
        "narrowed profile must exclude task objects from outgoing projection"
    );
    devices[4].1.set_selective_sync_profile(None);
    let persisted = devices[4].1.load_entities(&HashMap::new()).await;
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
