use super::SqliteStorageBackend;
use crate::application::sync_storage::{TaskMergeJournalPage, TaskMergeJournalPort};
use crate::db::{CommandClass, CommandContext, TaskMergeJournalPageQuery};

#[async_trait::async_trait]
impl TaskMergeJournalPort for SqliteStorageBackend {
    async fn task_merge_journal_page(
        &self,
        peer_device_id: &str,
        after_position: i64,
        through_position: i64,
        limit: usize,
    ) -> Result<TaskMergeJournalPage, String> {
        self.db
            .query(
                CommandContext::sync_peer(peer_device_id.to_owned(), CommandClass::SyncLive),
                TaskMergeJournalPageQuery::new(
                    peer_device_id.to_owned(),
                    after_position,
                    through_position,
                    limit,
                ),
            )
            .await
            .map_err(|error| error.to_string())
    }
}
