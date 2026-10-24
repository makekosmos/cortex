use super::super::executor::actor::{ClearAllTask, WriterTask};
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

}
