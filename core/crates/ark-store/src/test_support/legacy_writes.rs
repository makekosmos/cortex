use std::path::Path;

use ark_domain::types::VersionVector;
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyBatchRollbackState {
    pub object_exists: bool,
    pub revision_exists: bool,
    pub links_exist: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProjectDeleteState {
    pub deleted_at: Option<String>,
    pub tombstone_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyWriteSnapshot {
    pub version_vector: VersionVector,
    pub task_link_id: String,
    pub todo_tombstone_exists: bool,
    pub tracked_app_tombstone_exists: bool,
    pub legacy_todo_deleted_at: Option<String>,
    pub canonical_object_exists: bool,
    pub canonical_applied_receipt_count: i64,
    pub committed_task_feed_count: i64,
    pub task_merge_bootstrap_count: i64,
    pub task_existence_operation_count: i64,
}

/// Opens the service test database to install legacy-write failures and inspect rollback state.
pub struct LegacyWriteFixture {
    connection: Connection,
}

impl LegacyWriteFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        Ok(Self { connection })
    }

    pub fn inspect_successful_writes(&self) -> Result<LegacyWriteSnapshot, String> {
        let raw_vector = crate::sync_kv::get_sync_kv(&self.connection, "lan_sync.version_vector")?
            .ok_or_else(|| "legacy writes did not persist a version vector".to_string())?;
        let version_vector =
            serde_json::from_str(&raw_vector).map_err(|error| error.to_string())?;
        let task_link_id = self
            .connection
            .query_row(
                "SELECT id FROM object_links WHERE source_object_id = 'todo-legacy' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "legacy todo link was not written".to_string())?;
        let todo_tombstone_exists = self.tombstone_exists("todo-batch-1", "object")?;
        let tracked_app_tombstone_exists =
            self.tombstone_exists("tracked-app-legacy", "tracked_app")?;
        let legacy_todo_deleted_at = self
            .connection
            .query_row(
                "SELECT deleted_at FROM objects WHERE id = 'todo-batch-1'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .flatten();
        let canonical_object_exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM objects WHERE id = 'todo-canonical-cutover')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let canonical_applied_receipt_count = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM entity_write_receipts
                 WHERE entity_kind = 'object' AND entity_id = 'todo-canonical-cutover'
                   AND outcome = 'applied'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let committed_task_feed_count = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ark_committed_feed
                 WHERE entity_type = 'object' AND entity_id IN
                   ('todo-legacy', 'todo-batch-1', 'todo-batch-2', 'todo-canonical-cutover')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let task_merge_bootstrap_count = self
            .connection
            .query_row("SELECT COUNT(*) FROM task_merge_bootstraps", [], |row| {
                row.get(0)
            })
            .map_err(|error| error.to_string())?;
        let task_existence_operation_count = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM task_merge_operations
                 WHERE unit = '\"objectExistence\"'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        Ok(LegacyWriteSnapshot {
            version_vector,
            task_link_id,
            todo_tombstone_exists,
            tracked_app_tombstone_exists,
            legacy_todo_deleted_at,
            canonical_object_exists,
            canonical_applied_receipt_count,
            committed_task_feed_count,
            task_merge_bootstrap_count,
            task_existence_operation_count,
        })
    }

    pub fn install_todo_batch_failure(&self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "CREATE TRIGGER fail_legacy_batch BEFORE INSERT ON object_sync_versions
                 WHEN NEW.object_id = 'todo-fail-2'
                 BEGIN SELECT RAISE(ABORT, 'injected batch failure'); END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn inspect_batch_todo(&self, id: &str) -> Result<LegacyBatchRollbackState, String> {
        let object_exists =
            self.exists("SELECT EXISTS(SELECT 1 FROM objects WHERE id = ?1)", id)?;
        let revision_exists = self.exists(
            "SELECT EXISTS(SELECT 1 FROM object_sync_versions WHERE object_id = ?1)",
            id,
        )?;
        let links_exist = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM object_links
                 WHERE source_object_id = ?1 OR target_object_id = ?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        Ok(LegacyBatchRollbackState {
            object_exists,
            revision_exists,
            links_exist,
        })
    }

    pub fn install_project_delete_failure(&self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "CREATE TRIGGER fail_legacy_delete BEFORE INSERT ON sync_tombstones
                 WHEN NEW.id = 'project-legacy'
                 BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn inspect_project_delete(&self) -> Result<LegacyProjectDeleteState, String> {
        let deleted_at = self
            .connection
            .query_row(
                "SELECT deleted_at FROM objects WHERE id = 'project-legacy'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let tombstone_exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sync_tombstones WHERE id = 'project-legacy')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        Ok(LegacyProjectDeleteState {
            deleted_at,
            tombstone_exists,
        })
    }

    fn exists(&self, query: &str, id: &str) -> Result<bool, String> {
        self.connection
            .query_row(query, [id], |row| row.get(0))
            .map_err(|error| error.to_string())
    }

    fn tombstone_exists(&self, id: &str, entity_type: &str) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sync_tombstones
                 WHERE id = ?1 AND entity_type = ?2)",
                [id, entity_type],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }
}
