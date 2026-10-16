use crate::application::sync_storage::{
    PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort,
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
}

#[async_trait::async_trait]
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
        _: crate::application::sync_storage::SyncReadClass,
        _: &VersionVector,
        _: usize,
        _: usize,
    ) -> Result<Vec<SyncEntity>, String> {
        Ok(Vec::new())
    }
    async fn usage_complete_through(
        &self,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        Ok(std::collections::HashMap::new())
    }
    async fn usage_complete_through_for_peer(
        &self,
        _: &str,
        _: crate::application::sync_storage::SyncReadClass,
    ) -> Result<std::collections::HashMap<String, u64>, String> {
        Ok(std::collections::HashMap::new())
    }
    fn filter_outgoing_entity(&self, entity: &SyncEntity) -> Option<SyncEntity> {
        Some(entity.clone())
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
