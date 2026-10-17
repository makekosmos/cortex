use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ark_core::application::sync_storage::{
    PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort,
};
use ark_core::sync_server::{
    IntegrationReplicationBackend, PeerIdentityBackend, SyncRuntimeStorage,
};
use ark_core::types::{PeerRecord, SyncEntity, VersionVector};
use ark_core::SqliteStorageBackend;
use tokio::sync::{watch, Notify};

use super::{make_storage_with_identity, pick_port, SyncClient, SyncServer};

#[path = "six_devices/task_merge_port.rs"]
mod task_merge_port;

struct BackfillBarrier {
    inner: Arc<SqliteStorageBackend>,
    requested: Arc<Notify>,
    release: watch::Receiver<bool>,
}

#[async_trait::async_trait]
impl SyncEntityReadPort for BackfillBarrier {
    async fn load_entities(&self, vector: &VersionVector) -> Result<Vec<SyncEntity>, String> {
        self.inner.load_entities(vector).await
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        self.requested.notify_one();
        let mut release = self.release.clone();
        while !*release.borrow() {
            if release.changed().await.is_err() {
                return Ok(Vec::new());
            }
        }
        self.inner.load_entities_page(vector, offset, limit).await
    }

    async fn usage_complete_through(&self) -> Result<HashMap<String, u64>, String> {
        self.inner.usage_complete_through().await
    }

    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        self.inner.filter_outgoing_entity(entity)
    }
    async fn load_entities_page_for_peer(
        &self,
        peer: &str,
        class: ark_core::application::sync_storage::SyncReadClass,
        vector: &ark_core::VersionVector,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        self.requested.notify_one();
        let mut release = self.release.clone();
        while !*release.borrow() {
            if release.changed().await.is_err() {
                return Ok(Vec::new());
            }
        }
        self.inner
            .load_entities_page_for_peer(peer, class, vector, offset, limit)
            .await
    }
    async fn usage_complete_through_for_peer(
        &self,
        peer: &str,
        class: ark_core::application::sync_storage::SyncReadClass,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        self.inner
            .usage_complete_through_for_peer(peer, class)
            .await
    }
}
#[async_trait::async_trait]
impl SyncEntityMutationPort for BackfillBarrier {
    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        self.inner.apply_entity(entity).await
    }
}

#[async_trait::async_trait]
impl PeerProgressPort for BackfillBarrier {
    async fn load_version_vector(&self) -> Result<ark_core::VersionVector, String> {
        self.inner.load_version_vector().await
    }
    async fn store_version_vector(&self, value: &ark_core::VersionVector) -> Result<(), String> {
        self.inner.store_version_vector(value).await
    }
    async fn load_known_peers(&self) -> Result<Option<Vec<ark_core::PeerRecord>>, String> {
        self.inner.load_known_peers().await
    }
    async fn store_known_peers(&self, value: &[ark_core::PeerRecord]) -> Result<(), String> {
        self.inner.store_known_peers(value).await
    }
    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String> {
        self.inner.load_removed_peer_ids().await
    }
    async fn store_removed_peer_ids(&self, value: &[String]) -> Result<(), String> {
        self.inner.store_removed_peer_ids(value).await
    }
}

#[async_trait::async_trait]
impl ark_core::application::sync_storage::CommittedFeedPort for BackfillBarrier {
    fn selective_sync_policy_generation(&self) -> i64 {
        self.inner.selective_sync_policy_generation()
    }

    async fn committed_feed_snapshot(
        &self,
    ) -> Result<ark_core::application::sync_storage::FeedSnapshot, String> {
        self.inner.committed_feed_snapshot().await
    }

    async fn committed_feed_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<Vec<ark_core::application::sync_storage::CommittedFeedEntry>, String> {
        self.inner
            .committed_feed_page(after_position, high_watermark, limit)
            .await
    }

    async fn committed_feed_delivery_page(
        &self,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Result<ark_core::application::sync_storage::CommittedFeedDeliveryPage, String> {
        self.inner
            .committed_feed_delivery_page(after_position, high_watermark, limit)
            .await
    }

    async fn begin_outbound_policy(
        &self,
        peer: &str,
        generation: i64,
    ) -> Result<ark_core::application::sync_storage::OutboundProgress, String> {
        self.inner.begin_outbound_policy(peer, generation).await
    }
    async fn complete_outbound_snapshot(
        &self,
        peer: &str,
        generation: i64,
    ) -> Result<ark_core::application::sync_storage::OutboundProgress, String> {
        self.inner
            .complete_outbound_snapshot(peer, generation)
            .await
    }
    async fn record_outbound_scan(
        &self,
        peer: &str,
        generation: i64,
        through: i64,
    ) -> Result<ark_core::application::sync_storage::OutboundProgress, String> {
        self.inner
            .record_outbound_scan(peer, generation, through)
            .await
    }
    async fn acknowledge_outbound(
        &self,
        peer: &str,
        generation: i64,
        position: i64,
        ack: ark_core::application::sync_storage::OutboundAck,
    ) -> Result<ark_core::application::sync_storage::OutboundProgress, String> {
        self.inner
            .acknowledge_outbound(peer, generation, position, ack)
            .await
    }
}

#[async_trait::async_trait]
impl IntegrationReplicationBackend for BackfillBarrier {}

#[async_trait::async_trait]
impl PeerIdentityBackend for BackfillBarrier {
    async fn sync_session_is_admitted(&self, peer_device_id: &str) -> Result<bool, String> {
        self.inner.sync_session_is_admitted(peer_device_id).await
    }

    async fn authorized_transport_public_key(
        &self,
        device_id: &str,
    ) -> Result<Option<String>, String> {
        self.inner.authorized_transport_public_key(device_id).await
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        self.inner.load_peer_transport_keys().await
    }
    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        self.inner.store_peer_transport_keys(keys).await
    }

    async fn enroll_paired_device(
        &self,
        local_device_id: &str,
        target_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        self.inner
            .enroll_paired_device(local_device_id, target_device_id, transport_public_key)
            .await
    }

    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<
        (
            i64,
            usize,
            Vec<ark_core::domain::device_registry::RegistryEvent>,
        ),
        String,
    > {
        self.inner
            .load_device_registry_events_page(after_position, high_watermark, limit)
            .await
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[ark_core::domain::device_registry::RegistryEvent],
    ) -> Result<ark_core::replication_storage::RegistryPageApply, String> {
        self.inner
            .apply_device_registry_events(sender_device_id, events)
            .await
    }

    async fn sign_registry_handshake(
        &self,
        local_device_id: &str,
        transcript: &ark_core::domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_core::domain::device_registry::RegistryHandshakeProof, String> {
        self.inner
            .sign_registry_handshake(local_device_id, transcript)
            .await
    }

    async fn verify_registry_handshake(
        &self,
        peer_device_id: &str,
        transcript: &ark_core::domain::device_registry::RegistryHandshakeTranscript,
        proof: &ark_core::domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        self.inner
            .verify_registry_handshake(peer_device_id, transcript, proof)
            .await
    }
}

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
    // Deliberately drop the wake-up: the connected client's periodic durable
    // feed drain must recover this committed update without waiting for reconnect.
    release_tx.send(true).unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
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
