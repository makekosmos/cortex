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

    pub fn open_writer(&self) -> Result<SqliteWriter, StoreBootstrapError> {
        SqliteWriter::open_initialized(&self.path)
    }

    pub fn open_reader(&self) -> Result<SqliteReader, StoreError> {
        SqliteReader::open_read_only(&self.path)
    }
}
