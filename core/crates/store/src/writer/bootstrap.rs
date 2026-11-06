use super::SqliteWriter;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreBootstrapStage {
    Recovery,
    Open,
    Schema,
}

#[derive(Debug)]
pub struct StoreBootstrapError {
    pub stage: StoreBootstrapStage,
    message: String,
}

impl fmt::Display for StoreBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl SqliteWriter {
    /// Recover any interrupted restore and initialize the schema before the
    /// writer becomes available.
    pub fn open_initialized(path: &str) -> Result<Self, StoreBootstrapError> {
        crate::snapshot::recover_interrupted_restore(path).map_err(|error| {
            StoreBootstrapError {
                stage: StoreBootstrapStage::Recovery,
                message: error,
            }
        })?;

        let connection = rusqlite::Connection::open(path).map_err(|error| StoreBootstrapError {
            stage: StoreBootstrapStage::Open,
            message: error.to_string(),
        })?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|error| StoreBootstrapError {
                stage: StoreBootstrapStage::Open,
                message: error.to_string(),
            })?;
        crate::open::init_schema(&connection).map_err(|message| StoreBootstrapError {
            stage: StoreBootstrapStage::Schema,
            message,
        })?;
        Ok(Self::from_connection(connection))
    }
}
