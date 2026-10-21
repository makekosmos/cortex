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

        let connection = crate::open::open_db(path).map_err(|message| StoreBootstrapError {
            stage: StoreBootstrapStage::Open,
            message,
        })?;
        crate::open::init_schema(&connection).map_err(|message| StoreBootstrapError {
            stage: StoreBootstrapStage::Schema,
            message,
        })?;

        let mut writer = Self::from_connection(connection);
        crate::canonical_migrations::rewrite_legacy_tasks(
            &mut writer,
            migration_backup_path,
            bootstrap_device_id,
        )
        .map_err(|message| StoreBootstrapError {
            stage: StoreBootstrapStage::Migration,
            message,
        })?;
        Ok(writer)
    }
}
