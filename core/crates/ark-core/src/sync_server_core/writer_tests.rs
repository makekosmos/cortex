use ark_application::sync_storage::{
    PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort, TaskMergeDeliveryPort,
};
use std::sync::atomic::{AtomicBool, Ordering};

struct RevocableBackend {
    allowed: AtomicBool,
}

#[async_trait::async_trait]
impl IntegrationReplicationBackend for RevocableBackend {
    async fn validate_outbound_signed_integration_frame(
        &self,
        _frame: &SignedSyncEnvelope,
        _expected_space_id: &str,
        _expected_origin_node_id: &str,
    ) -> Result<(), String> {
        self.allowed
            .load(Ordering::SeqCst)
            .then_some(())
            .ok_or_else(|| "revoked".to_string())
    }

    async fn apply_signed_integration_frame(
        &self,
        _frame: &SignedSyncEnvelope,
        _expected_space_id: &str,
        _authenticated_peer_id: &str,
        _expected_recipient_node_id: &str,
    ) -> Result<(), String> {
        Err("signed integration replication unsupported in test backend".into())
    }
}

#[async_trait::async_trait]
impl SyncEntityReadPort for RevocableBackend {
    async fn load_entities(&self, _: &VersionVector) -> Result<Vec<SyncEntity>, String> {
        Ok(Vec::new())
    }
    async fn load_entities_page(
        &self,
        _: &VersionVector,
        _: usize,
        _: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        Ok(Vec::new())
    }
    async fn load_entities_page_for_peer(
        &self,
        _: &str,
        _: ark_application::sync_storage::SyncReadClass,
        _: &VersionVector,
        _: usize,
        _: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        Ok(Vec::new())
    }
    async fn load_policy_bound_entities_page_for_peer(
        &self,
        _peer_device_id: &str,
        _class: ark_application::sync_storage::SyncReadClass,
        _vector: &VersionVector,
        _offset: usize,
        _limit: usize,
        expected_identity: &ark_application::sync_storage::PolicyIdentity,
    ) -> Result<
        ark_application::sync_storage::PolicyBoundSnapshotPage,
        ark_application::sync_storage::PolicyBoundSnapshotReadError,
    > {
        let accepted_identity = ark_application::sync_policy::default_sync_policy_identity();
        if expected_identity != &accepted_identity {
            return Err(ark_application::sync_storage::PolicyBoundSnapshotReadError::StalePolicy);
        }
        Ok(ark_application::sync_storage::PolicyBoundSnapshotPage {
            identity: accepted_identity,
            entities: Vec::new(),
        })
    }
    async fn usage_complete_through(
        &self,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        Ok(std::collections::HashMap::new())
    }
    async fn usage_complete_through_for_peer(
        &self,
        _: &str,
        _: ark_application::sync_storage::SyncReadClass,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        Ok(std::collections::HashMap::new())
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
impl SyncEntityMutationPort for RevocableBackend {
    async fn apply_entity(&self, _entity: &SyncEntity) -> Result<(), String> {
        Ok(())
    }
}
#[async_trait::async_trait]
impl PeerProgressPort for RevocableBackend {
    async fn load_version_vector(&self) -> Result<VersionVector, String> {
        Ok(VersionVector::new())
    }
    async fn store_version_vector(&self, _: &VersionVector) -> Result<(), String> {
        Ok(())
    }
    async fn store_version_vector_with_policy(
        &self,
        _local_device_id: &str,
        _policy_identity: &ark_application::sync_storage::PolicyIdentity,
        _vector: &VersionVector,
    ) -> Result<(), String> {
        Ok(())
    }
    async fn load_known_peers(&self) -> Result<Option<Vec<crate::types::PeerRecord>>, String> {
        Ok(Some(Vec::new()))
    }
    async fn store_known_peers(&self, _: &[crate::types::PeerRecord]) -> Result<(), String> {
        Ok(())
    }
    async fn load_removed_peer_ids(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    async fn store_removed_peer_ids(&self, _: &[String]) -> Result<(), String> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl ark_application::sync_storage::PeerSyncPolicyPort for RevocableBackend {
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
impl TaskMergeDeliveryPort for RevocableBackend {
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
impl ark_application::sync_storage::CommittedFeedPort for RevocableBackend {
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
impl AuthenticatedEntityBackend for RevocableBackend {}

#[async_trait::async_trait]
impl PeerIdentityBackend for RevocableBackend {
    async fn authorized_transport_public_key(&self, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        Ok(std::collections::HashMap::new())
    }
    async fn store_peer_transport_keys(
        &self,
        _: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn enroll_paired_device(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn queued_frame_is_dropped_when_revoked_before_writer_recheck() {
    let backend_impl = Arc::new(RevocableBackend {
        allowed: AtomicBool::new(true),
    });
    let backend = backend_impl.clone() as Arc<dyn SyncRuntimeStorage>;
    let frame =
        SignedSyncEnvelope::new("space", "origin", "recipient", 1, "message", vec![], "sig");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(frame).unwrap();
    backend_impl.allowed.store(false, Ordering::SeqCst);

    let queued = rx.recv().await.expect("queued frame");
    assert!(
        !signed_integration_write_is_authorized(&backend, &queued, "space", "origin",).await,
        "writer must drop a frame revoked after enqueue"
    );
}
