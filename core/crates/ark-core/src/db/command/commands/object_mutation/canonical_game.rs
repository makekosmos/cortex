use super::super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::object_mutation::apply_game_upsert;
use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::db::SqliteObjectMutationSession;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CanonicalGameUpsertCommand {
    command: GameUpsertCommand,
    fallback_device_id: String,
}

impl CanonicalGameUpsertCommand {
    pub(crate) fn new(command: GameUpsertCommand, fallback_device_id: String) -> Self {
        Self {
            command,
            fallback_device_id,
        }
    }
}

impl sealed::Sealed for CanonicalGameUpsertCommand {}

impl DbCommand for CanonicalGameUpsertCommand {
    type Output = GameMutationResult;

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
        let session = SqliteObjectMutationSession::new(transaction);
        apply_game_upsert(&session, self.command, &self.fallback_device_id)
            .map_err(DbError::from_application_error)
    }
}
