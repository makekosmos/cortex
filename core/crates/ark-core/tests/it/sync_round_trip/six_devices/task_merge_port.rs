use super::BackfillBarrier;
use ark_core::application::sync_storage::{TaskMergeJournalPage, TaskMergeJournalPort};
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
impl TaskMergeJournalPort for BackfillBarrier {
    async fn task_merge_journal_page(
        &self,
        peer_device_id: &str,
        after_position: i64,
        through_position: i64,
        limit: usize,
    ) -> Result<TaskMergeJournalPage, String> {
        self.inner
            .task_merge_journal_page(peer_device_id, after_position, through_position, limit)
            .await
    }
}
