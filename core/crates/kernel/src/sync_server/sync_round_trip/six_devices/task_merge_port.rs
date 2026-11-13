use super::BackfillBarrier;
use ::domain::task_merge::TaskMergeDeliveryPort;
use ::domain::task_merge_sync::TaskMergeBatch;
use kernel::sync_server::AuthenticatedEntityBackend;
use kernel::types::SyncEntity;

#[async_trait::async_trait]
impl AuthenticatedEntityBackend for BackfillBarrier {
    async fn receive_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        peer: &str,
        message_id: &str,
    ) -> Result<bool, String> {
        self.inner
            .receive_authenticated_entity_with_id(entity, peer, message_id)
            .await
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        peer: &str,
        message_id: &str,
        local_device_id: &str,
        policy: Option<::domain::sync_policy::PolicyIdentity>,
    ) -> Result<bool, String> {
        self.inner
            .receive_authenticated_entity_with_policy(
                entity,
                peer,
                message_id,
                local_device_id,
                policy,
            )
            .await
    }

    async fn apply_authenticated_entity(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
    ) -> Result<(), String> {
        self.inner
            .apply_authenticated_entity(entity, authenticated_peer_id)
            .await
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        peer: &str,
        message_id: &str,
    ) -> Result<::domain::sync_storage::InboundApplyOutcome, String> {
        self.inner
            .apply_authenticated_entity_with_id(entity, peer, message_id)
            .await
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        peer: &str,
        message_id: &str,
        local_device_id: &str,
        policy: Option<::domain::sync_policy::PolicyIdentity>,
    ) -> Result<::domain::sync_storage::InboundApplyOutcome, String> {
        self.inner
            .apply_authenticated_entity_with_policy(
                entity,
                peer,
                message_id,
                local_device_id,
                policy,
            )
            .await
    }
}

#[async_trait::async_trait]
impl ::domain::sync_storage::PeerSyncPolicyPort for BackfillBarrier {
    async fn accepted_policy_identity_for_peer(
        &self,
        peer_device_id: &str,
    ) -> Result<Option<::domain::sync_policy::PolicyIdentity>, String> {
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
        expected_identity: &::domain::sync_policy::PolicyIdentity,
        limit: usize,
    ) -> Result<Option<TaskMergeBatch>, ::domain::sync_storage::TaskMergePrepareError> {
        self.inner
            .prepare_task_merge_batch(peer_device_id, expected_identity, limit)
            .await
    }

    async fn acknowledge_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &::domain::sync_policy::PolicyIdentity,
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
        expected_identity: &::domain::sync_policy::PolicyIdentity,
        batch: &TaskMergeBatch,
    ) -> Result<::domain::task_merge_sync::TaskMergeBatchAck, String> {
        self.inner
            .apply_task_merge_batch(peer_device_id, local_device_id, expected_identity, batch)
            .await
    }
}
