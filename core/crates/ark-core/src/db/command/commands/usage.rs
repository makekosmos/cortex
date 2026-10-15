use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::usage::{apply_local_usage_write, LocalUsageWrite, UsageMutation};
use crate::db::SqliteUsageWriteSession;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalUsageWriteCommand {
    device_id: String,
    write: LocalUsageWrite,
}

impl LocalUsageWriteCommand {
    pub(crate) fn new(device_id: String, write: LocalUsageWrite) -> Self {
        Self { device_id, write }
    }
}

impl sealed::Sealed for LocalUsageWriteCommand {}

impl DbCommand for LocalUsageWriteCommand {
    type Output = Vec<UsageMutation>;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        serde_json::to_vec(self)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let session = SqliteUsageWriteSession::new(transaction);
        apply_local_usage_write(&session, &self.device_id, self.write)
            .map_err(DbError::from_application_error)
    }
}
