use super::*;

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    // Minimal in-memory backend for unit tests.
    pub(crate) struct MemBackend {
        kv: StdMutex<HashMap<String, String>>,
        entities: StdMutex<HashMap<String, SyncEntity>>,
    }

    impl MemBackend {
        pub(crate) fn new() -> Self {
            Self {
                kv: StdMutex::new(HashMap::new()),
                entities: StdMutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl StorageBackend for MemBackend {
        async fn validate_outbound_signed_integration_frame(
            &self,
            _frame: &SignedSyncEnvelope,
            _expected_space_id: &str,
            _expected_origin_node_id: &str,
        ) -> Result<(), String> {
            Ok(())
        }

        async fn load_entities(&self, _vector: &VersionVector) -> Vec<SyncEntity> {
            self.entities.lock().unwrap().values().cloned().collect()
        }
        async fn load_entities_page(
            &self,
            _vector: &VersionVector,
            offset: usize,
            limit: usize,
        ) -> Vec<SyncEntity> {
            self.entities
                .lock()
                .unwrap()
                .values()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect()
        }
        async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
            if entity.deleted == Some(true) {
                self.entities.lock().unwrap().remove(&entity.id);
            } else {
                self.entities
                    .lock()
                    .unwrap()
                    .insert(entity.id.clone(), entity.clone());
            }
            Ok(())
        }
        async fn get_kv(&self, key: &str) -> Option<String> {
            self.kv.lock().unwrap().get(key).cloned()
        }
        async fn set_kv(&self, key: &str, value: &str) {
            self.kv
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
    }

    fn peer_rec(device_id: &str, addrs: &[&str]) -> PeerRecord {
        PeerRecord {
            device_id: device_id.to_string(),
            device_name: format!("{device_id}-name"),
            addresses: addrs.iter().map(|s| s.to_string()).collect(),
            last_seen: "2026-04-01T00:00:00.000Z".to_string(),
            last_address: None,
            platform: None,
            app_version: None,
        }
    }

    #[tokio::test]
    async fn purge_self_peers_drops_our_device_id() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["192.168.1.10:21531".to_string()];

        *server.known_peer_records.lock().await = vec![
            peer_rec("me", &["192.168.1.10:21531"]),
            peer_rec("other", &["192.168.1.20:21531"]),
        ];

        let removed = server.purge_self_peers().await;
        assert_eq!(removed, 1);
        let known = server.get_known_peers().await;
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].device_id, "other");
    }

    #[tokio::test]
    async fn purge_self_peers_drops_records_with_all_our_addresses() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec![
            "192.168.1.10:21531".to_string(),
            "10.0.0.5:21531".to_string(),
        ];

        *server.known_peer_records.lock().await = vec![
            // Different device_id, but every address is ours -> phantom self.
            peer_rec("phantom", &["192.168.1.10:21531", "10.0.0.5:21531"]),
            // Keep: one of the addresses is not ours.
            peer_rec("real", &["192.168.1.10:21531", "192.168.1.99:21531"]),
        ];

        let removed = server.purge_self_peers().await;
        assert_eq!(removed, 1);
        let ids: Vec<String> = server
            .get_known_peers()
            .await
            .into_iter()
            .map(|p| p.device_id)
            .collect();
        assert_eq!(ids, vec!["real".to_string()]);
    }

    fn peer_record(device_id: &str, name: &str, addrs: &[&str]) -> PeerRecord {
        PeerRecord {
            device_id: device_id.to_string(),
            device_name: name.to_string(),
            addresses: addrs.iter().map(|a| a.to_string()).collect(),
            last_seen: "2026-04-01T00:00:00.000Z".to_string(),
            last_address: None,
            platform: None,
            app_version: None,
        }
    }

    #[tokio::test]
    async fn register_external_peer_rejects_self() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["10.0.0.1:21531".to_string()];

        server
            .register_external_peer(peer_record("me", "Me", &["10.0.0.1:21531"]))
            .await;
        // Also: all-self addresses under a different device_id.
        server
            .register_external_peer(peer_record("phantom", "Phantom", &["10.0.0.1:21531"]))
            .await;

        assert!(
            server.get_known_peers().await.is_empty(),
            "register_external_peer must drop self and all-self-address records",
        );
    }

    #[tokio::test]
    async fn register_external_peer_stores_routable_peer() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["10.0.0.1:21531".to_string()];

        server
            .register_external_peer(peer_record("other", "Other", &["192.168.1.20:21531"]))
            .await;

        let known = server.get_known_peers().await;
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].device_id, "other");
        assert_eq!(known[0].addresses, vec!["192.168.1.20:21531".to_string()]);
    }

    #[tokio::test]
    async fn block_peer_persists_removed_peer_and_filters_known_list() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.known_peer_records.lock().await = vec![peer_rec("other", &["192.168.1.20:21531"])];

        server.block_peer("other").await;

        assert!(server.get_known_peers().await.is_empty());
        assert_eq!(
            server.get_removed_peer_ids().await,
            vec!["other".to_string()]
        );
        assert!(server.is_peer_blocked("other").await);
    }

    #[tokio::test]
    async fn register_external_peer_ignores_blocked_peer() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        *server.device_id.write().await = "me".to_string();
        *server.own_addresses.write().await = vec!["10.0.0.1:21531".to_string()];
        server.block_peer("blocked").await;

        server
            .register_external_peer(peer_record("blocked", "Blocked", &["192.168.1.21:21531"]))
            .await;

        assert!(server.get_known_peers().await.is_empty());
    }

    #[tokio::test]
    async fn disconnect_peer_closes_matching_sessions_and_persists_blocklist() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());

        let (tx1, _rx1) = mpsc::unbounded_channel::<Message>();
        let (tx2, _rx2) = mpsc::unbounded_channel::<Message>();
        let (tx3, _rx3) = mpsc::unbounded_channel::<Message>();

        server.peers.lock().await.insert(
            1,
            PeerState {
                device_id: "blocked".to_string(),
                device_name: "Blocked One".to_string(),
                addresses: vec!["192.168.1.20:21531".to_string()],
                platform: None,
                app_version: None,
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx: tx1,
            },
        );
        server.peers.lock().await.insert(
            2,
            PeerState {
                device_id: "blocked".to_string(),
                device_name: "Blocked Two".to_string(),
                addresses: vec!["192.168.1.21:21531".to_string()],
                platform: None,
                app_version: None,
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx: tx2,
            },
        );
        server.peers.lock().await.insert(
            3,
            PeerState {
                device_id: "keep".to_string(),
                device_name: "Keep".to_string(),
                addresses: vec!["192.168.1.22:21531".to_string()],
                platform: None,
                app_version: None,
                authenticated: true,
                sync_complete: true,
                queued_live_changes: vec![],
                tx: tx3,
            },
        );

        let closed = server.disconnect_peer("blocked").await;
        assert_eq!(closed, 2);
        assert_eq!(
            server.get_removed_peer_ids().await,
            vec!["blocked".to_string()]
        );
        assert_eq!(server.connected_peer_count().await, 1);
        assert_eq!(
            server.get_connected_peer_entries().await,
            vec![crate::sync_server::PeerEntry {
                device_id: "keep".to_string(),
                device_name: "Keep".to_string(),
                platform: None,
                app_version: None,
            }]
        );
        assert_eq!(server.peers.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn get_connected_peer_entries_dedup_by_device_id() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn StorageBackend>;
        let server = SyncServer::new(storage.clone());
        // Simulate two authenticated sessions from the same device_id (a race
        // between old + new sessions before eviction settles).
        let (tx, _rx) = mpsc::unbounded_channel::<Message>();
        let state = |device_id: &str| PeerState {
            device_id: device_id.to_string(),
            device_name: "DupDevice".to_string(),
            addresses: vec![],
            platform: None,
            app_version: None,
            authenticated: true,
            sync_complete: true,
            queued_live_changes: vec![],
            tx: tx.clone(),
        };
        server.peers.lock().await.insert(1, state("dup"));
        server.peers.lock().await.insert(2, state("dup"));

        let entries = server.get_connected_peer_entries().await;
        assert_eq!(
            entries.len(),
            1,
            "duplicate device_id sessions should collapse to one entry",
        );
        assert_eq!(entries[0].device_id, "dup");
    }
}
