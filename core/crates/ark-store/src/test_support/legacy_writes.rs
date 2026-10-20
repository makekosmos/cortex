use std::path::Path;

use rusqlite::Connection;

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

/// Opens the service test database to install legacy-write failures and inspect rollback state.
pub struct LegacyWriteFailureFixture {
    connection: Connection,
}

impl LegacyWriteFailureFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        Ok(Self { connection })
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
}
