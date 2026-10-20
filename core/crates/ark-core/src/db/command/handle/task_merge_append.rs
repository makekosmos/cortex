use super::*;
#[cfg(debug_assertions)]
use crate::db::command::executor::actor::map_append_operation_error;
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
        match &self.backend {
            HandleBackend::Actor(actor) => {
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
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_task_merge_append(Arc::clone(connection), context, command, admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_task_merge_append(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    command: super::super::AppendTaskMergeOperationRequest,
    admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<TaskMergeAppendResult, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        context.require_engine()?;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::append_local_task_merge_operation_on_connection(
            &mut connection,
            command.operation,
            command.expected_conflict_digest,
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_store_error(error),
            ark_store::writer::TransactionFailure::Operation(error) => {
                map_append_operation_error(error)
            }
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })
    })
}
