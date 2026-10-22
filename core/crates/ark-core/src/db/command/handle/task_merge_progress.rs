use super::super::executor::actor::{
    AcknowledgeTaskMergeBatchTask, PrepareTaskMergeBatchTask, WriterTask,
};
use super::super::{AcknowledgeTaskMergeBatchCommand, PrepareTaskMergeBatchCommand};
use super::*;

impl DbHandle {
    pub(crate) async fn prepare_task_merge_batch(
        &self,
        context: CommandContext,
        command: PrepareTaskMergeBatchCommand,
    ) -> Result<Option<ark_domain::task_merge_sync::TaskMergeBatch>, DbError> {
        let weight = command.weight();
        validate_sync_task_merge_request(&context, &command.peer_id, weight)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(PrepareTaskMergeBatchTask { command, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn acknowledge_task_merge_batch(
        &self,
        context: CommandContext,
        command: AcknowledgeTaskMergeBatchCommand,
    ) -> Result<i64, DbError> {
        let weight = command.weight();
        validate_sync_task_merge_request(&context, &command.peer_id, weight)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> =
                Box::new(AcknowledgeTaskMergeBatchTask { command, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}

fn validate_sync_task_merge_request(
    context: &CommandContext,
    peer_id: &str,
    weight: usize,
) -> Result<(), DbError> {
    if context.class != CommandClass::SyncLive {
        return Err(DbError::Invalid("database command class mismatch".into()));
    }
    context.require_sync_peer_matches(peer_id)?;
    if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
        return Err(DbError::TooLarge);
    }
    Ok(())
}
