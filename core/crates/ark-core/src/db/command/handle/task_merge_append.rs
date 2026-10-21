use super::*;
use crate::db::command::executor::actor::AppendTaskMergeOperationTask;
use ark_application::task_merge::TaskMergeAppendResult;

impl DbHandle {
    pub(crate) async fn append_task_merge_operation(
        &self,
        context: CommandContext,
        command: super::super::AppendTaskMergeOperationRequest,
    ) -> Result<TaskMergeAppendResult, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(AppendTaskMergeOperationTask {
                context: context.clone(),
                operation: command.operation,
                expected_conflict_digest: command.expected_conflict_digest,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
