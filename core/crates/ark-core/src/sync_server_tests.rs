use super::*;
use ark_application::sync_storage::{
    PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort, TaskMergeDeliveryPort,
};

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    // Minimal in-memory backend for unit tests.
    pub(crate) struct MemBackend {
        kv: StdMutex<HashMap<String, String>>,
        entities: StdMutex<HashMap<String, SyncEntity>>,
        authenticated_apply_error: StdMutex<Option<String>>,
        peer_transport_keys_error: StdMutex<Option<String>>,
        outbound_snapshot_identity_completions: AtomicUsize,
    }

    impl MemBackend {
        pub(crate) fn new() -> Self {
            Self {
                kv: StdMutex::new(HashMap::new()),
                entities: StdMutex::new(HashMap::new()),
                authenticated_apply_error: StdMutex::new(None),
                peer_transport_keys_error: StdMutex::new(None),
                outbound_snapshot_identity_completions: AtomicUsize::new(0),
            }
        }

        pub(crate) fn outbound_snapshot_identity_completion_count(&self) -> usize {
            self.outbound_snapshot_identity_completions
                .load(Ordering::SeqCst)
        }

        pub(crate) fn fail_authenticated_apply(&self, error: &str) {
            *self.authenticated_apply_error.lock().unwrap() = Some(error.to_string());
        }

        pub(crate) fn fail_peer_transport_keys_read(&self, error: &str) {
            *self.peer_transport_keys_error.lock().unwrap() = Some(error.to_string());
        }

        pub(crate) fn set_sync_kv(&self, key: &str, value: &str) {
            self.kv
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
    }

    #[async_trait::async_trait]
    impl SyncEntityReadPort for MemBackend {
        async fn load_entities(&self, _vector: &VersionVector) -> Result<Vec<SyncEntity>, String> {
            Ok(self.entities.lock().unwrap().values().cloned().collect())
        }

        async fn load_entities_page(
            &self,
            _vector: &VersionVector,
            offset: usize,
            limit: usize,
        ) -> Result<Vec<SyncEntity>, String> {
            Ok(self
                .entities
                .lock()
                .unwrap()
                .values()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect())
        }
        async fn load_entities_page_for_peer(
            &self,
            _peer_device_id: &str,
            _class: ark_application::sync_storage::SyncReadClass,
            vector: &VersionVector,
            offset: usize,
            limit: usize,
        ) -> Result<Vec<SyncEntity>, String> {
            self.load_entities_page(vector, offset, limit).await
        }
        async fn load_policy_bound_entities_page_for_peer(
            &self,
            peer_device_id: &str,
            class: ark_application::sync_storage::SyncReadClass,
            vector: &VersionVector,
            offset: usize,
            limit: usize,
            expected_identity: &ark_application::sync_storage::PolicyIdentity,
        ) -> Result<ark_application::sync_storage::PolicyBoundSnapshotPage, String> {
            let accepted_identity = ark_application::sync_policy::default_sync_policy_identity();
            if expected_identity != &accepted_identity {
                return Err("stale_or_unnegotiated_sync_policy".into());
            }
            let entities = self
                .load_entities_page_for_peer(peer_device_id, class, vector, offset, limit)
                .await?;
            Ok(ark_application::sync_storage::PolicyBoundSnapshotPage {
                identity: accepted_identity,
                entities,
            })
        }
        async fn usage_complete_through(&self) -> Result<HashMap<String, u64>, String> {
            Ok(HashMap::new())
        }
        async fn usage_complete_through_for_peer(
            &self,
            _: &str,
            _: ark_application::sync_storage::SyncReadClass,
        ) -> Result<HashMap<String, u64>, String> {
            Ok(HashMap::new())
        }
        fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
            Some(entity.clone())
        }
        async fn filter_outgoing_entity_for_peer(
            &self,
            _peer_device_id: &str,
            entity: &SyncEntity,
        ) -> Result<Option<SyncEntity>, String> {
            Ok(self.filter_outgoing_entity(entity))
        }
    }
    #[async_trait::async_trait]
    impl SyncEntityMutationPort for MemBackend {
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
    }

    #[async_trait::async_trait]
    impl ark_application::sync_storage::PeerSyncPolicyPort for MemBackend {
        async fn accepted_policy_identity_for_peer(
            &self,
            _peer_device_id: &str,
        ) -> Result<Option<ark_application::sync_storage::PolicyIdentity>, String> {
            Ok(Some(
                ark_application::sync_policy::default_sync_policy_identity(),
            ))
        }
    }

    #[async_trait::async_trait]
    impl TaskMergeDeliveryPort for MemBackend {
        async fn prepare_task_merge_batch(
            &self,
            _peer_device_id: &str,
            _expected_identity: &ark_application::sync_storage::PolicyIdentity,
            _limit: usize,
        ) -> Result<
            Option<ark_application::sync_storage::TaskMergeBatch>,
            ark_application::task_merge::TaskMergePrepareError,
        > {
            Ok(None)
        }

        async fn acknowledge_task_merge_batch(
            &self,
            _peer_device_id: &str,
            _expected_identity: &ark_application::sync_storage::PolicyIdentity,
            _batch_id: &str,
            through_position: i64,
        ) -> Result<i64, String> {
            Ok(through_position)
        }

        async fn apply_task_merge_batch(
            &self,
            _peer_device_id: &str,
            _local_device_id: &str,
            _expected_identity: &ark_application::sync_storage::PolicyIdentity,
            _batch: &ark_application::sync_storage::TaskMergeBatch,
        ) -> Result<ark_application::sync_storage::TaskMergeBatchAck, String> {
            Err("test_task_merge_receive_unsupported".into())
        }
    }
    #[async_trait::async_trait]
    impl PeerProgressPort for MemBackend {
        async fn load_version_vector(&self) -> Result<VersionVector, String> {
            self.kv
                .lock()
                .unwrap()
                .get(crate::db::VERSION_VECTOR_KEY)
                .and_then(|raw| serde_json::from_str(raw).ok())
                .map_or_else(
                    || Ok(VersionVector::new()),
                    |raw| serde_json::from_str(raw).map_err(|error| error.to_string()),
                )
        }
        async fn store_version_vector(&self, vector: &VersionVector) -> Result<(), String> {
            self.kv.lock().unwrap().insert(
                crate::db::VERSION_VECTOR_KEY.into(),
                serde_json::to_string(vector).unwrap(),
            );
            Ok(())
        }
        async fn load_known_peers(&self) -> Result<Option<Vec<PeerRecord>>, String> {
            self.kv
                .lock()
                .unwrap()
                .get(crate::db::constants::KNOWN_PEERS_KEY)
                .map(|raw| serde_json::from_str(raw).map_err(|error| error.to_string()))
                .transpose()
        }
        async fn store_known_peers(&self, peers: &[PeerRecord]) -> Result<(), String> {
            self.kv.lock().unwrap().insert(
                crate::db::constants::KNOWN_PEERS_KEY.into(),
                serde_json::to_string(peers).unwrap(),
            );
            Ok(())
        }
        async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String> {
            self.kv
                .lock()
                .unwrap()
                .get(crate::db::constants::REMOVED_PEERS_KEY)
                .map_or_else(
                    || Ok(Vec::new()),
                    |raw| serde_json::from_str(raw).map_err(|error| error.to_string()),
                )
        }
        async fn store_removed_peer_ids(&self, ids: &[String]) -> Result<(), String> {
            self.kv.lock().unwrap().insert(
                crate::db::constants::REMOVED_PEERS_KEY.into(),
                serde_json::to_string(ids).unwrap(),
            );
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl ark_application::sync_storage::CommittedFeedPort for MemBackend {
        fn selective_sync_policy_generation(&self) -> i64 {
            0
        }

        async fn committed_feed_snapshot(
            &self,
        ) -> Result<ark_application::sync_storage::FeedSnapshot, String> {
            Err("test_feed_unsupported".into())
        }

        async fn committed_feed_page(
            &self,
            _: i64,
            _: i64,
            _: usize,
        ) -> Result<Vec<ark_application::sync_storage::CommittedFeedEntry>, String> {
            Err("test_feed_unsupported".into())
        }

        async fn committed_feed_delivery_page(
            &self,
            _: i64,
            _: i64,
            _: usize,
        ) -> Result<ark_application::sync_storage::CommittedFeedDeliveryPage, String> {
            Err("test_feed_unsupported".into())
        }

        async fn begin_outbound_policy(
            &self,
            _: &str,
            _: i64,
        ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
            Ok(ark_application::sync_storage::OutboundProgress {
                initial_snapshot: ark_application::sync_storage::InitialSnapshot::Complete {
                    through: 0,
                },
                ..Default::default()
            })
        }
        async fn complete_outbound_snapshot(
            &self,
            _: &str,
            _: i64,
        ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
            Ok(ark_application::sync_storage::OutboundProgress {
                initial_snapshot: ark_application::sync_storage::InitialSnapshot::Complete {
                    through: 0,
                },
                ..Default::default()
            })
        }
        async fn complete_outbound_snapshot_identity(
            &self,
            _: &str,
            _: &ark_application::sync_storage::PolicyIdentity,
        ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
            self.outbound_snapshot_identity_completions
                .fetch_add(1, Ordering::SeqCst);
            Err("test_policy_identity_completion_observed".into())
        }
        async fn record_outbound_scan(
            &self,
            _: &str,
            _: i64,
            _: i64,
        ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
            Err("test_feed_unsupported".into())
        }
        async fn acknowledge_outbound(
            &self,
            _: &str,
            _: i64,
            _: i64,
            _: ark_application::sync_storage::OutboundAck,
        ) -> Result<ark_application::sync_storage::OutboundProgress, String> {
            Err("test_feed_unsupported".into())
        }
    }

    #[async_trait::async_trait]
    impl IntegrationReplicationBackend for MemBackend {
        async fn validate_outbound_signed_integration_frame(
            &self,
            _frame: &SignedSyncEnvelope,
            _expected_space_id: &str,
            _expected_origin_node_id: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl PeerIdentityBackend for MemBackend {
        async fn authorized_transport_public_key(
            &self,
            _device_id: &str,
        ) -> Result<Option<String>, String> {
            Ok(None)
        }

        async fn load_peer_transport_keys(&self) -> Result<HashMap<String, String>, String> {
            if let Some(error) = self.peer_transport_keys_error.lock().unwrap().clone() {
                return Err(error);
            }
            self.kv
                .lock()
                .unwrap()
                .get(crate::db::constants::PEER_TRANSPORT_KEYS_KEY)
                .map_or_else(
                    || Ok(HashMap::new()),
                    |raw| serde_json::from_str(raw).map_err(|error| error.to_string()),
                )
        }
        async fn store_peer_transport_keys(
            &self,
            keys: &HashMap<String, String>,
        ) -> Result<(), String> {
            self.kv.lock().unwrap().insert(
                crate::db::constants::PEER_TRANSPORT_KEYS_KEY.into(),
                serde_json::to_string(keys).unwrap(),
            );
            Ok(())
        }

        async fn enroll_paired_device(
            &self,
            _local_device_id: &str,
            target_device_id: &str,
            transport_public_key: &str,
        ) -> Result<(), String> {
            let mut keys = self.load_peer_transport_keys().await?;
            if keys
                .get(target_device_id)
                .is_some_and(|existing| existing != transport_public_key)
            {
                return Err("test transport key conflict".into());
            }
            keys.insert(target_device_id.into(), transport_public_key.into());
            self.store_peer_transport_keys(&keys).await
        }
    }

    #[async_trait::async_trait]
    impl AuthenticatedEntityBackend for MemBackend {
        async fn apply_authenticated_entity(
            &self,
            entity: &SyncEntity,
            _authenticated_peer_id: &str,
        ) -> Result<(), String> {
            if let Some(error) = self.authenticated_apply_error.lock().unwrap().clone() {
                return Err(error);
            }
            self.apply_entity(entity).await
        }
    }

    include!("sync_server_tests/peer_management.rs");
}
