use super::super::{sealed, CommandContext, DbError, DbMaintenanceCommand};

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

impl CompactUsageSyncLogCommand {
    pub(in crate::db::command) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    pub(in crate::db::command) fn execute(
        self,
        context: &CommandContext,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<usize, DbError> {
        context.require_engine()?;
        writer
            .compact_usage_sync_log(
                self.own_device_id.as_deref(),
                &self.cutoff_wall_time,
                self.batch_limit,
            )
            .map_err(map_usage_compaction_failure)
    }
}

fn map_usage_compaction_failure(
    failure: ark_store::writer::StoreTransactionFailure<String>,
) -> DbError {
    use ark_store::writer::{StoreErrorKind, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => DbError::Storage(error),
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
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
    type Output = ark_store::snapshot::RestoreReport;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<Self::Output, DbError> {
        writer
            .restore_snapshot(&self.db_path, &self.backup_id)
            .map_err(map_restore_error)
    }
}

fn map_restore_error(error: String) -> DbError {
    if error.starts_with("restore recovery required:") {
        DbError::RecoveryRequired
    } else if let Some(reason) = error.strip_prefix("restore refused:") {
        DbError::RestoreRefused(reason.trim().to_string())
    } else {
        DbError::RestoreFailed(error)
    }
}

#[cfg(test)]
mod restore_error_tests {
    use super::map_restore_error;
    use crate::db::command::DbError;

    #[test]
    fn restore_recovery_required_errors_are_typed_for_fail_closed_actor_handling() {
        assert!(matches!(
            map_restore_error("restore recovery required: rollback failed".into()),
            DbError::RecoveryRequired
        ));
        assert!(matches!(
            map_restore_error("restore refused: stale snapshot".into()),
            DbError::RestoreRefused(reason) if reason == "stale snapshot"
        ));
        assert!(matches!(
            map_restore_error("snapshot open failed".into()),
            DbError::RestoreFailed(message) if message == "snapshot open failed"
        ));
    }
}
