use super::super::executor::actor::{
    AcknowledgeTaskMergeBatchTask, PrepareTaskMergeBatchTask, WriterTask,
};
use super::super::{AcknowledgeTaskMergeBatchCommand, PrepareTaskMergeBatchCommand};
use super::*;
#[cfg(test)]
use ark_store::writer::SqliteWriter;

impl DbHandle {
    pub(crate) async fn prepare_task_merge_batch(
        &self,
        context: CommandContext,
        command: PrepareTaskMergeBatchCommand,
    ) -> Result<Option<ark_application::sync_storage::TaskMergeBatch>, DbError> {
        let weight = command.weight();
        validate_sync_task_merge_request(&context, &command.peer_id, weight)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> =
                    Box::new(PrepareTaskMergeBatchTask { command, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                let connection = Arc::clone(connection);
                tokio::task::spawn_blocking(move || {
                    let _admission = admission;
                    let mut connection =
                        connection.lock().unwrap_or_else(|error| error.into_inner());
                    SqliteWriter::prepare_outbound_task_merge_batch_on_connection(
                        &mut connection,
                        &command.peer_id,
                        command.expected_identity,
                        command.limit,
                    )
                    .map_err(super::super::executor::actor::map_task_merge_progress_failure)
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn acknowledge_task_merge_batch(
        &self,
        context: CommandContext,
        command: AcknowledgeTaskMergeBatchCommand,
    ) -> Result<i64, DbError> {
        let weight = command.weight();
        validate_sync_task_merge_request(&context, &command.peer_id, weight)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> =
                    Box::new(AcknowledgeTaskMergeBatchTask { command, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                let connection = Arc::clone(connection);
                tokio::task::spawn_blocking(move || {
                    let _admission = admission;
                    let mut connection =
                        connection.lock().unwrap_or_else(|error| error.into_inner());
                    SqliteWriter::acknowledge_outbound_task_merge_batch_on_connection(
                        &mut connection,
                        &command.peer_id,
                        command.expected_identity,
                        &command.batch_id,
                        command.through_position,
                    )
                    .map_err(super::super::executor::actor::map_task_merge_progress_failure)
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
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
