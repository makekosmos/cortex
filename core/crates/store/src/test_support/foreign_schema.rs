use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// Owns a valid SQLite file with a schema that is not an ARK schema.
///
/// The connection is closed after creation, while callers can only obtain the
/// path to exercise higher-level snapshot validation.
pub struct ForeignSchemaFixture {
    path: PathBuf,
}

impl ForeignSchemaFixture {
    /// Creates an incompatible SQLite database at `path`.
    pub fn create(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let connection = Connection::open(&path).map_err(|error| error.to_string())?;
        connection
            .execute_batch("CREATE TABLE other(id TEXT PRIMARY KEY)")
            .map_err(|error| error.to_string())?;
        drop(connection);
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
