use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
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
