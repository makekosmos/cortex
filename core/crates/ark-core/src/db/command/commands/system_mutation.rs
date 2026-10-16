use super::super::{
    sealed, CommandClass, CommandContext, DbCommand, DbError, DbMaintenanceCommand,
};
use rusqlite::Transaction;

pub(crate) struct ClearAllCommand;

impl sealed::Sealed for ClearAllCommand {}

impl DbCommand for ClearAllCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Maintenance;

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::db::clear_all(transaction).map_err(DbError::Storage)
    }
}

pub(crate) struct DeleteTrashedCommand {
    device_id: String,
}

impl DeleteTrashedCommand {
    pub(crate) fn new(device_id: String) -> Self {
        Self { device_id }
    }
}

impl sealed::Sealed for DeleteTrashedCommand {}

impl DbCommand for DeleteTrashedCommand {
    type Output = usize;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        self.device_id.len().max(1)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::db::delete_trashed(transaction, &self.device_id).map_err(DbError::Storage)
    }
}

#[derive(serde::Serialize)]
pub(crate) struct CompactUsageSyncLogCommand {
    own_device_id: Option<String>,
    cutoff_wall_time: String,
    batch_limit: i64,
}

impl CompactUsageSyncLogCommand {
    pub(crate) fn new(
        own_device_id: Option<String>,
        cutoff_wall_time: String,
        batch_limit: i64,
    ) -> Self {
        Self {
            own_device_id,
            cutoff_wall_time,
            batch_limit,
        }
    }
}

impl sealed::Sealed for CompactUsageSyncLogCommand {}

impl DbCommand for CompactUsageSyncLogCommand {
    type Output = usize;

    const CLASS: CommandClass = CommandClass::Maintenance;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::db::compact_usage_sync_log(
            transaction,
            self.own_device_id.as_deref(),
            &self.cutoff_wall_time,
            self.batch_limit,
        )
        .map_err(DbError::Storage)
    }
}

#[derive(serde::Serialize)]
pub(crate) struct RestoreSnapshotCommand {
    db_path: String,
    backup_id: String,
}

impl RestoreSnapshotCommand {
    pub(crate) fn new(db_path: String, backup_id: String) -> Self {
        Self { db_path, backup_id }
    }
}

impl sealed::Sealed for RestoreSnapshotCommand {}

impl DbMaintenanceCommand for RestoreSnapshotCommand {
    type Output = crate::db::RestoreReport;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(self, connection: &mut rusqlite::Connection) -> Result<Self::Output, DbError> {
        crate::db::restore_snapshot(connection, &self.db_path, &self.backup_id)
            .map_err(DbError::Storage)
    }
}
