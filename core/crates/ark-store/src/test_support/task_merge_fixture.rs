use std::path::Path;

use ark_domain::task_merge::{
    TaskConflictSet, TaskMergeBootstrap, TaskMergeOperation, TaskMergeUnit,
};
use ark_domain::types::ArkObject;
use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeState {
    pub object: Option<ArkObject>,
    pub bootstrap: Option<TaskMergeBootstrap>,
    pub title_conflicts: Option<TaskConflictSet>,
    pub operations: Vec<TaskMergeOperation>,
    pub existence_operations: Vec<TaskMergeOperation>,
    pub operation_count: i64,
    pub journal_count: i64,
    pub operation_journal_count: i64,
    pub operation_journal: Vec<(i64, String)>,
    pub receipt_count: i64,
    pub link_count: i64,
    pub tombstone_count: i64,
    pub revision_count: i64,
    pub committed_feed_count: i64,
    pub version_vector_has_object: bool,
}

/// Owns storage fault injection and persistence inspection for task-merge tests.
pub struct TaskMergeFixture {
    connection: Connection,
}

impl TaskMergeFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "task-merge database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        Ok(Self { connection })
    }

    pub fn reject_operation_journal_writes(&self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "CREATE TRIGGER reject_local_task_journal
                 BEFORE INSERT ON task_merge_journal
                 WHEN NEW.record_kind = 'operation'
                 BEGIN SELECT RAISE(ABORT, 'injected task journal failure'); END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn reject_all_journal_writes(&self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "CREATE TRIGGER reject_task_merge_journal
                 BEFORE INSERT ON task_merge_journal
                 BEGIN SELECT RAISE(ABORT, 'injected merge persistence failure'); END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn inspect(&self, object_id: &str) -> Result<TaskMergeState, String> {
        let object = crate::objects::get_object(&self.connection, object_id)?;
        let bootstrap = crate::task_merge::has_bootstrap(&self.connection, object_id)
            .map_err(|error| error.to_string())?
            .then(|| crate::task_merge::load_bootstrap(&self.connection, object_id))
            .transpose()
            .map_err(|error| error.to_string())?;
        let title_conflicts =
            crate::task_merge::read_conflict_set(&self.connection, object_id, TaskMergeUnit::Title)
                .map_err(|error| error.to_string())?
                .conflict_set;
        let unit = serde_json::to_string(&TaskMergeUnit::ObjectExistence)
            .map_err(|error| error.to_string())?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT operation_json FROM task_merge_operations
                 WHERE object_id = ?1 AND unit = ?2 ORDER BY operation_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(rusqlite::params![object_id, unit], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| error.to_string())?;
        let existence_operations = rows
            .map(|row| {
                let json = row.map_err(|error| error.to_string())?;
                serde_json::from_str(&json).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        drop(statement);
        let mut statement = self
            .connection
            .prepare(
                "SELECT operation_json FROM task_merge_operations
                 WHERE object_id = ?1 ORDER BY operation_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([object_id], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        let operations = rows
            .map(|row| {
                let json = row.map_err(|error| error.to_string())?;
                serde_json::from_str(&json).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        drop(statement);
        let operation_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM task_merge_operations WHERE object_id = ?1",
            object_id,
        )?;
        let journal_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM task_merge_journal
             WHERE record_id IN (
                 SELECT bootstrap_id FROM task_merge_bootstraps WHERE object_id = ?1
             )",
            object_id,
        )?;
        let operation_journal_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM task_merge_journal
             WHERE record_kind = 'operation' AND record_id IN (
                 SELECT operation_id FROM task_merge_operations WHERE object_id = ?1
             )",
            object_id,
        )?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT position, record_id FROM task_merge_journal
                 WHERE record_kind = 'operation' AND record_id IN (
                     SELECT operation_id FROM task_merge_operations WHERE object_id = ?1
                 ) ORDER BY position",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([object_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| error.to_string())?;
        let operation_journal = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        drop(statement);
        let receipt_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
            object_id,
        )?;
        let link_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM object_links WHERE source_object_id = ?1",
            object_id,
        )?;
        let tombstone_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1",
            object_id,
        )?;
        let revision_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
            object_id,
        )?;
        let committed_feed_count = count(
            &self.connection,
            "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
            object_id,
        )?;
        let vector = crate::sync_kv::get_sync_kv(&self.connection, crate::VERSION_VECTOR_KEY)?;
        let version_vector_has_object = vector
            .as_deref()
            .map(serde_json::from_str::<ark_domain::types::VersionVector>)
            .transpose()
            .map_err(|error| error.to_string())?
            .is_some_and(|value| value.contains_key(object_id));

        Ok(TaskMergeState {
            object,
            bootstrap,
            title_conflicts,
            operations,
            existence_operations,
            operation_count,
            journal_count,
            operation_journal_count,
            operation_journal,
            receipt_count,
            link_count,
            tombstone_count,
            revision_count,
            committed_feed_count,
            version_vector_has_object,
        })
    }
}

fn count(connection: &Connection, sql: &str, object_id: &str) -> Result<i64, String> {
    connection
        .query_row(sql, [object_id], |row| row.get(0))
        .map_err(|error| error.to_string())
}
