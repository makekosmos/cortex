//! File-backed database fixtures for tests that exercise Core's real writer.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

/// Owns a temporary ARK database for the full lifetime of a test.
///
/// The database is initialized before callers start a writer actor. Tests may
/// open a separate connection for setup or read-only assertions, but should
/// route writes through the actor once it is running.
pub struct TestDatabase {
    directory: TempDir,
    path: PathBuf,
}

impl TestDatabase {
    pub fn create() -> Result<Self, String> {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let path = directory.path().join("ark.db");
        let path_text = path
            .to_str()
            .ok_or_else(|| "temporary database path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path_text)?;
        crate::open::init_schema(&connection)?;
        drop(connection);
        Ok(Self { directory, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn open_connection(&self) -> Result<Connection, String> {
        let path = self
            .path
            .to_str()
            .ok_or_else(|| "temporary database path is not valid UTF-8".to_string())?;
        crate::open::open_db(path)
    }

    /// Keeps the temporary directory's ownership explicit at call sites.
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }
}

#[cfg(test)]
mod tests {
    use super::TestDatabase;

    #[test]
    fn creates_a_reopenable_initialized_database() {
        let database = TestDatabase::create().expect("database initializes");
        let connection = database.open_connection().expect("database reopens");
        let table_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'objects'
                )",
                [],
                |row| row.get(0),
            )
            .expect("schema query succeeds");

        assert!(database.path().exists());
        assert_eq!(
            database.directory(),
            database
                .path()
                .parent()
                .expect("database path has a parent")
        );
        assert!(table_exists);
    }
}
