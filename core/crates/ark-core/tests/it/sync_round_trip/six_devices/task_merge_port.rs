use super::BackfillBarrier;
use ark_application::sync_storage::{TaskMergeBatch, TaskMergeDeliveryPort};
use ark_core::sync_server::AuthenticatedEntityBackend;
use ark_core::types::SyncEntity;

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for BackfillBarrier {
    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String> {
        self.inner
            .apply_authenticated_entity(entity, authenticated_peer_id)
            .await
    }
}

#[async_trait::async_trait]
impl ark_application::sync_storage::PeerSyncPolicyPort for BackfillBarrier {
    async fn accepted_policy_identity_for_peer(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<ark_application::sync_storage::PolicyIdentity>, String> {
        self.inner
            .accepted_policy_identity_for_peer(peer_device_id)
            .await
    }
}

#[async_trait::async_trait]
impl TaskMergeDeliveryPort for BackfillBarrier {
    async fn prepare_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &ark_application::sync_storage::PolicyIdentity,
        limit: usize,
    ) -> Result<Option<TaskMergeBatch>, ark_application::sync_storage::TaskMergePrepareError> {
        self.inner
            .prepare_task_merge_batch(peer_device_id, expected_identity, limit)
            .await
    }

    async fn acknowledge_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &ark_application::sync_storage::PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, String> {
        self.inner
            .acknowledge_task_merge_batch(
                peer_device_id,
                expected_identity,
                batch_id,
                through_position,
            )
            .await
    }

    async fn apply_task_merge_batch(
        &self,
        peer_device_id: &str,
        local_device_id: &str,
        expected_identity: &ark_application::sync_storage::PolicyIdentity,
        batch: &TaskMergeBatch,
    ) -> Result<ark_application::sync_storage::TaskMergeBatchAck, String> {
        self.inner
            .apply_task_merge_batch(peer_device_id, local_device_id, expected_identity, batch)
            .await
    }
}
