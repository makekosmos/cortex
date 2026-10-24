use super::SqliteWriter;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreBootstrapStage {
    Recovery,
    Open,
    Schema,
    Migration,
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
    /// Recover any interrupted restore, initialize the schema, and apply the
    /// required canonical migration before the writer becomes available.
    pub fn open_initialized(
        path: &str,
        migration_backup_path: &str,
        bootstrap_device_id: &str,
    ) -> Result<Self, StoreBootstrapError> {
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
        crate::core_migration::backup_before_migration(&connection, migration_backup_path).map_err(
            |message| StoreBootstrapError {
                stage: StoreBootstrapStage::Migration,
                message,
            },
        )?;
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
        let mut connection = connection;
        crate::core_migration::migrate(&mut connection, bootstrap_device_id).map_err(|message| {
            StoreBootstrapError {
            stage: StoreBootstrapStage::Migration,
            message,
            }
        })?;
        Ok(Self::from_connection(connection))
    }
}
