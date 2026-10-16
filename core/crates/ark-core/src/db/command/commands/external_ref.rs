use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::data_platform::{self, ExternalRefUpsert};
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct UpsertExternalRefCommand {
    request: ExternalRefUpsert,
}

impl UpsertExternalRefCommand {
    pub(crate) fn new(request: ExternalRefUpsert) -> Self {
        Self { request }
    }
}

impl sealed::Sealed for UpsertExternalRefCommand {}

impl DbCommand for UpsertExternalRefCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        data_platform::ensure_schema(transaction).map_err(DbError::from_application_error)?;
        data_platform::upsert_external_ref(transaction, &self.request)
            .map_err(DbError::from_application_error)
    }
}
