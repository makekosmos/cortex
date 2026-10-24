use crate::read_snapshot::SqliteReader;
use crate::writer::{SqliteWriter, StoreBootstrapError, StoreError};

#[derive(Clone, Debug)]
pub struct StoreDatabase {
    path: String,
}

impl StoreDatabase {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn open_writer(
        &self,
        bootstrap_device_id: &str,
    ) -> Result<SqliteWriter, StoreBootstrapError> {
        let migration_backup_path = format!("{}.pre-core.bak", self.path);
        SqliteWriter::open_initialized(&self.path, &migration_backup_path, bootstrap_device_id)
    }

    pub fn open_reader(&self) -> Result<SqliteReader, StoreError> {
        SqliteReader::open_read_only(&self.path)
    }
}
