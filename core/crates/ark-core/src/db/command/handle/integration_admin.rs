use super::*;
use crate::db::command::{IntegrationAdminCommand, IntegrationAdminOutput};

impl DbHandle {
    pub(crate) async fn submit_integration_admin(
        &self,
        context: CommandContext,
        command: IntegrationAdminCommand,
    ) -> Result<IntegrationAdminOutput, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(executor::actor::IntegrationAdminTask {
                context: context.clone(),
                command,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
