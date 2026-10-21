use super::super::executor::actor::{ApplyTaskMergeBatchTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn apply_task_merge_batch(
        &self,
        context: CommandContext,
        command: super::super::ApplyTaskMergeBatchCommand,
    ) -> Result<ark_domain::task_merge_sync::TaskMergeBatchAck, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(ApplyTaskMergeBatchTask {
                context: context.clone(),
                command,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
