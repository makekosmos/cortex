use super::SqliteStorageBackend;
use crate::application::sync_storage::{
    PolicyIdentity, TaskMergeBatch, TaskMergeBatchAck, TaskMergeDeliveryPort,
};
use crate::db::{
    AcknowledgeTaskMergeBatchCommand, ApplyTaskMergeBatchCommand, CommandClass, CommandContext,
    PrepareTaskMergeBatchCommand,
};

#[async_trait::async_trait]
impl TaskMergeDeliveryPort for SqliteStorageBackend {
    async fn prepare_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        limit: usize,
    ) -> Result<Option<TaskMergeBatch>, ark_application::task_merge::TaskMergePrepareError> {
        self.db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                PrepareTaskMergeBatchCommand::new(
                    peer_device_id.to_owned(),
                    expected_identity.clone(),
                    limit,
                ),
            )
            .await
            .map_err(|error| match error {
                crate::db::DbError::Permission(_) => {
                    ark_application::task_merge::TaskMergePrepareError::PolicyRejected
                }
                error => {
                    ark_application::task_merge::TaskMergePrepareError::Failure(error.to_string())
                }
            })
    }

    async fn acknowledge_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, String> {
        self.db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                AcknowledgeTaskMergeBatchCommand::new(
                    peer_device_id.to_owned(),
                    expected_identity.clone(),
                    batch_id.to_owned(),
                    through_position,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_task_merge_batch(
        &self,
        peer_device_id: &str,
        local_device_id: &str,
        expected_identity: &PolicyIdentity,
        batch: &TaskMergeBatch,
    ) -> Result<TaskMergeBatchAck, String> {
        self.db
            .submit(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                ApplyTaskMergeBatchCommand::new(
                    peer_device_id.to_owned(),
                    local_device_id.to_owned(),
                    expected_identity.clone(),
                    batch.clone(),
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }
}
