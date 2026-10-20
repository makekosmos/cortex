use super::super::executor::actor::{LocalCoreTaskWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_local_core_task_write(
        &self,
        context: CommandContext,
        operation_id: Option<String>,
        command: LocalCoreTaskWrite,
    ) -> Result<TaskWriteOperationResult, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        if operation_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        #[derive(serde::Serialize)]
        struct OperationWrite<'a> {
            operation_id: &'a str,
            command_type: &'static str,
            command: &'a LocalCoreTaskWrite,
        }
        let weight = operation_id.as_deref().map_or_else(
            || command.weight(),
            |operation_id| {
                serde_json::to_vec(&OperationWrite {
                    operation_id,
                    command_type: "object.upsert.v1",
                    command: &command,
                })
                .map_or(usize::MAX, |bytes| bytes.len())
            },
        );
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LocalCoreTaskWriteTask {
                    context: context.clone(),
                    command,
                    operation_id,
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
                spawn_test_local_core_task_write(
                    Arc::clone(connection),
                    context,
                    operation_id,
                    command,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_local_core_task_write(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    operation_id: Option<String>,
    command: LocalCoreTaskWrite,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<TaskWriteOperationResult, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        context.require_engine()?;
        if command.object_type_id() != "core.task" {
            return Err(DbError::Unsupported(
                "object_write_slice_unsupported_type".into(),
            ));
        }
        let principal = context.operation_principal();
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::apply_local_core_task_write_on_connection(
            &mut connection,
            command,
            &principal,
            operation_id.as_deref(),
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
            ark_store::writer::TransactionFailure::Operation(
                ark_store::writer::TaskObjectWriteError::OperationIdConflict,
            ) => DbError::Conflict(
                "operationId was already used with a different command payload".into(),
            ),
            ark_store::writer::TransactionFailure::Operation(
                ark_store::writer::TaskObjectWriteError::Storage(error),
            ) => DbError::from_application_error(error),
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })
    })
}
