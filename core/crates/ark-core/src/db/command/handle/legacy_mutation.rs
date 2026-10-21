use super::super::executor::actor::{LegacyDeleteTask, LegacyGraphWriteTask, WriterTask};
use super::super::{LegacyDeleteCommand, LegacyGraphWriteCommand};
use super::*;

impl DbHandle {
    pub(crate) async fn write_legacy_graph(
        &self,
        context: CommandContext,
        command: LegacyGraphWriteCommand,
    ) -> Result<Vec<crate::types::SyncEntity>, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = command.weight();
        validate_legacy_write_weight(weight)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LegacyGraphWriteTask { command, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn delete_legacy_object(
        &self,
        context: CommandContext,
        command: LegacyDeleteCommand,
    ) -> Result<crate::types::SyncEntity, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = command.weight();
        validate_legacy_write_weight(weight)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LegacyDeleteTask { command, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}

fn validate_legacy_write_weight(weight: usize) -> Result<(), DbError> {
    if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
        return Err(DbError::TooLarge);
    }
    Ok(())
}
