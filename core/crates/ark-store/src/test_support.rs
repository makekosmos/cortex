//! File-backed database fixtures for tests that exercise Core's real writer.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Deserialize;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreIntentState {
    Missing,
    Corrupt,
    Valid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptedRestoreState {
    pub intent: RestoreIntentState,
    pub capsule_present: bool,
}

/// Owns the files and marker data used to exercise actor startup recovery.
pub struct InterruptedRestoreFixture {
    database: TestDatabase,
    intent_path: PathBuf,
    capsule_path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreIntent {
    #[serde(rename = "version")]
    _version: u32,
    operation_id: String,
    #[serde(rename = "capsule_sha256")]
    _capsule_sha256: String,
}

impl InterruptedRestoreFixture {
    pub fn create() -> Result<Self, String> {
        let database = TestDatabase::create()?;
        let path = database
            .path()
            .to_str()
            .ok_or_else(|| "temporary database path is not valid UTF-8".to_string())?;
        let connection = database.open_connection()?;
        connection
            .execute_batch(
                "CREATE TABLE restore_journal_test(value TEXT NOT NULL);
                 INSERT INTO restore_journal_test VALUES ('before');",
            )
            .map_err(|error| error.to_string())?;
        crate::snapshot::leave_restore_journal_for_test(&connection, path)?;
        connection
            .execute("UPDATE restore_journal_test SET value = 'partial'", [])
            .map_err(|error| error.to_string())?;

        let intent_path = database_path_with_suffix(&database, "restore-intent.json");
        let intent: RestoreIntent =
            serde_json::from_slice(&fs::read(&intent_path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        validate_restore_intent(&intent)?;
        let capsule_path = database_path_with_suffix(
            &database,
            &format!("restore-live-{}.db", intent.operation_id),
        );
        Ok(Self {
            database,
            intent_path,
            capsule_path,
        })
    }

    pub fn database_path(&self) -> &Path {
        self.database.path()
    }

    pub fn corrupt_intent(&self) -> Result<(), String> {
        fs::write(&self.intent_path, b"not-json").map_err(|error| error.to_string())
    }

    pub fn remove_capsule(&self) -> Result<(), String> {
        fs::remove_file(&self.capsule_path).map_err(|error| error.to_string())
    }

    pub fn inspect(&self) -> Result<InterruptedRestoreState, String> {
        let bytes = match fs::read(&self.intent_path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        let intent = match bytes.as_deref() {
            None => RestoreIntentState::Missing,
            Some(bytes) => match serde_json::from_slice::<RestoreIntent>(bytes) {
                Ok(intent) if validate_restore_intent(&intent).is_ok() => RestoreIntentState::Valid,
                _ => RestoreIntentState::Corrupt,
            },
        };
        Ok(InterruptedRestoreState {
            intent,
            capsule_present: self.capsule_path.exists(),
        })
    }

    pub fn marker(&self) -> Result<String, String> {
        self.marker_on(&self.database.open_connection()?)
    }

    pub fn marker_on(&self, connection: &Connection) -> Result<String, String> {
        connection
            .query_row("SELECT value FROM restore_journal_test", [], |row| {
                row.get(0)
            })
            .map_err(|error| error.to_string())
    }
}

fn validate_restore_intent(intent: &RestoreIntent) -> Result<(), String> {
    if intent._version != 1
        || intent.operation_id.len() != 16
        || !intent
            .operation_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || intent._capsule_sha256.len() != 64
        || !intent
            ._capsule_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("restore fixture intent fields are invalid".into());
    }
    Ok(())
}

fn database_path_with_suffix(database: &TestDatabase, suffix: &str) -> PathBuf {
    let file_name = database
        .path()
        .file_name()
        .expect("database has a file name")
        .to_string_lossy();
    database
        .path()
        .parent()
        .expect("database has a parent")
        .join(format!(".{file_name}.{suffix}"))
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
