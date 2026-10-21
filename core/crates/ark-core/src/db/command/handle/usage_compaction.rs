use super::super::commands::CompactUsageSyncLogCommand;
use super::super::executor::actor::{UsageCompactionTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn compact_usage_sync_log(
        &self,
        context: CommandContext,
        command: CompactUsageSyncLogCommand,
    ) -> Result<usize, DbError> {
        if context.class != CommandClass::Maintenance {
            return Err(DbError::Invalid(
                "maintenance command class required".into(),
            ));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(UsageCompactionTask {
                context: context.clone(),
                command,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
