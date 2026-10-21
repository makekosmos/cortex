use super::super::executor::actor::{ClearAllTask, DeleteTrashedTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_clear_all(&self, context: CommandContext) -> Result<(), DbError> {
        if context.class != CommandClass::Maintenance {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(ClearAllTask { reply });
            actor.admit(context.class, &context.source, 1, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_delete_trashed(
        &self,
        context: CommandContext,
        device_id: String,
    ) -> Result<usize, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = device_id.len().max(1);
        if weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(DeleteTrashedTask { device_id, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
