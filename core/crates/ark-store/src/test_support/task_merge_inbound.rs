use std::path::Path;

use ark_application::sync_storage::TaskMergeJournalEntry;
use ark_domain::types::{ArkObject, ObjectLink};
use rusqlite::OptionalExtension;

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeInboundState {
    pub object: Option<ArkObject>,
    pub links: Vec<ObjectLink>,
    pub operation_count: i64,
    pub bootstrap_count: i64,
    pub journal: Vec<TaskMergeJournalEntry>,
    pub inbound_cursor: Option<i64>,
    pub inbound_progress_count: i64,
    pub resolution_guard: Option<String>,
}

/// Owns task-merge test setup and persistence inspection for inbound batches.
pub struct TaskMergeInboundFixture {
    connection: rusqlite::Connection,
}

impl TaskMergeInboundFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "task-merge database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        Ok(Self { connection })
    }

    pub fn seed_object(&self, object: &ArkObject) -> Result<(), String> {
        crate::objects::upsert_object_compat(&self.connection, object)
    }

    pub fn inspect(
        &self,
        object_id: &str,
        peer_id: &str,
        operation_id: &str,
    ) -> Result<TaskMergeInboundState, String> {
        let object = crate::objects::get_object(&self.connection, object_id)?;
        let links = crate::objects::list_object_links(&self.connection)?
            .into_iter()
            .filter(|link| link.source_object_id == object_id)
            .collect();
        let operation_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM task_merge_operations",
        )?;
        let bootstrap_count = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_bootstraps WHERE object_id = ?1",
                [object_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let through_position = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(position), 0) FROM task_merge_journal",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let journal = crate::task_merge::read_journal_page(
            &self.connection,
            0,
            through_position,
            ark_application::task_merge::MAX_TASK_MERGE_PAGE_SIZE,
        )
        .map_err(|error| error.to_string())?
        .entries;
        let inbound_cursor = self
            .connection
            .query_row(
                "SELECT received_through FROM task_merge_inbound_progress
                 WHERE peer_device_id = ?1",
                [peer_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let inbound_progress_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM task_merge_inbound_progress",
        )?;
        let resolution_guard = self
            .connection
            .query_row(
                "SELECT expected_conflict_digest FROM task_merge_resolution_guards
                 WHERE operation_id = ?1",
                [operation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;

        Ok(TaskMergeInboundState {
            object,
            links,
            operation_count,
            bootstrap_count,
            journal,
            inbound_cursor,
            inbound_progress_count,
            resolution_guard,
        })
    }
}

fn count(connection: &rusqlite::Connection, sql: &str) -> Result<i64, String> {
    connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(|error| error.to_string())
}
