use super::super::executor::actor::{
    CanonicalGameUpsertTask, LocalBookCoverTask, LocalObjectDeleteTask, LocalObjectUpsertTask,
    WriterTask,
};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_canonical_game_upsert(
        &self,
        context: CommandContext,
        command: ark_application::canonical_game::GameUpsertCommand,
        fallback_device_id: String,
    ) -> Result<ark_application::canonical_game::GameMutationResult, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        #[derive(serde::Serialize)]
        struct GameCommand<'a> {
            command: &'a ark_application::canonical_game::GameUpsertCommand,
            fallback_device_id: &'a str,
        }
        let weight = serde_json::to_vec(&GameCommand {
            command: &command,
            fallback_device_id: &fallback_device_id,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(CanonicalGameUpsertTask {
                    command,
                    fallback_device_id,
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
                spawn_test_canonical_game_upsert(
                    Arc::clone(connection),
                    command,
                    fallback_device_id,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn submit_local_book_cover(
        &self,
        context: CommandContext,
        write: ark_application::book_cover::LocalBookCoverWrite,
    ) -> Result<bool, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        #[derive(serde::Serialize)]
        struct BookCoverCommand<'a> {
            write: &'a ark_application::book_cover::LocalBookCoverWrite,
        }
        let weight = serde_json::to_vec(&BookCoverCommand { write: &write })
            .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }

        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LocalBookCoverTask { write, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_local_book_cover(Arc::clone(connection), write, admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn submit_local_object_upsert(
        &self,
        context: CommandContext,
        write: ark_application::object_mutation::LocalObjectUpsert,
        operation_id: Option<String>,
    ) -> Result<
        super::super::operation_receipt::OperationResult<
            ark_application::object_mutation::ObjectUpsertCommit,
        >,
        DbError,
    > {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        if operation_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        #[derive(serde::Serialize)]
        struct UpsertCommand<'a> {
            write: &'a ark_application::object_mutation::LocalObjectUpsert,
        }
        #[derive(serde::Serialize)]
        struct ReceiptCommand<'a> {
            operation_id: &'a Option<String>,
            command_type: &'static str,
            command: UpsertCommand<'a>,
        }
        let weight = serde_json::to_vec(&ReceiptCommand {
            operation_id: &operation_id,
            command_type: "object.upsert.v1",
            command: UpsertCommand { write: &write },
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }

        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LocalObjectUpsertTask {
                    write,
                    principal: context.operation_principal(),
                    operation_id,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                let outcome = response.await.unwrap_or(Err(DbError::OutcomeUnknown))?;
                Ok(super::super::operation_receipt::OperationResult {
                    value: outcome.value,
                    replayed: outcome.replayed,
                })
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                let outcome = spawn_test_local_object_upsert(
                    Arc::clone(connection),
                    write,
                    context.operation_principal(),
                    operation_id,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))??;
                Ok(super::super::operation_receipt::OperationResult {
                    value: outcome.value,
                    replayed: outcome.replayed,
                })
            }
        }
    }

    pub(crate) async fn submit_local_object_delete(
        &self,
        context: CommandContext,
        write: ark_application::object_mutation::LocalObjectDelete,
        operation_id: Option<String>,
    ) -> Result<
        super::super::operation_receipt::OperationResult<
            ark_application::object_mutation::ObjectDeleteCommit,
        >,
        DbError,
    > {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        if operation_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        #[derive(serde::Serialize)]
        struct DeleteCommand<'a> {
            write: &'a ark_application::object_mutation::LocalObjectDelete,
        }
        #[derive(serde::Serialize)]
        struct ReceiptCommand<'a> {
            operation_id: &'a Option<String>,
            command_type: &'static str,
            command: DeleteCommand<'a>,
        }
        let weight = serde_json::to_vec(&ReceiptCommand {
            operation_id: &operation_id,
            command_type: "object.delete.v1",
            command: DeleteCommand { write: &write },
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }

        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LocalObjectDeleteTask {
                    write,
                    principal: context.operation_principal(),
                    operation_id,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                let (value, replayed) = response.await.unwrap_or(Err(DbError::OutcomeUnknown))?;
                Ok(super::super::operation_receipt::OperationResult { value, replayed })
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                let outcome = spawn_test_local_object_delete(
                    Arc::clone(connection),
                    write,
                    context.operation_principal(),
                    operation_id,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))??;
                Ok(super::super::operation_receipt::OperationResult {
                    value: outcome.0,
                    replayed: outcome.1,
                })
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_canonical_game_upsert(
    connection: Arc<Mutex<Connection>>,
    command: ark_application::canonical_game::GameUpsertCommand,
    fallback_device_id: String,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<ark_application::canonical_game::GameMutationResult, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::canonical_game::apply_upsert(&mut connection, command, &fallback_device_id)
            .map_err(|failure| match failure {
                ark_store::writer::TransactionFailure::Begin(error) => {
                    DbError::from_store_error(error)
                }
                ark_store::writer::TransactionFailure::Operation(error) => {
                    DbError::from_application_error(error.to_string())
                }
                ark_store::writer::TransactionFailure::Commit(error) => {
                    DbError::CommitFailed(error.to_string())
                }
            })
    })
}

#[cfg(debug_assertions)]
fn spawn_test_local_book_cover(
    connection: Arc<Mutex<Connection>>,
    write: ark_application::book_cover::LocalBookCoverWrite,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<bool, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::book_cover::apply_local_book_cover(&mut connection, write).map_err(|failure| {
            match failure {
                ark_store::writer::TransactionFailure::Begin(error) => {
                    DbError::from_store_error(error)
                }
                ark_store::writer::TransactionFailure::Operation(error) => {
                    DbError::from_application_error(error.to_string())
                }
                ark_store::writer::TransactionFailure::Commit(error) => {
                    DbError::CommitFailed(error.to_string())
                }
            }
        })
    })
}

#[cfg(debug_assertions)]
fn spawn_test_local_object_upsert(
    connection: Arc<Mutex<Connection>>,
    write: ark_application::object_mutation::LocalObjectUpsert,
    principal: String,
    operation_id: Option<String>,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<ark_application::object_mutation::ObjectUpsertOutcome, DbError>>
{
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        let outcome = ark_store::object_write::apply_local_object_upsert(
            &mut connection,
            write,
            &principal,
            operation_id.as_deref(),
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_store_error(error),
            ark_store::writer::TransactionFailure::Operation(error) => {
                DbError::from_application_error(error.to_string())
            }
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })?;
        Ok(outcome)
    })
}

#[cfg(debug_assertions)]
fn spawn_test_local_object_delete(
    connection: Arc<Mutex<Connection>>,
    write: ark_application::object_mutation::LocalObjectDelete,
    principal: String,
    operation_id: Option<String>,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<
    Result<(ark_application::object_mutation::ObjectDeleteCommit, bool), DbError>,
> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::object_mutation::apply_local_object_delete(
            &mut connection,
            write,
            &principal,
            operation_id.as_deref(),
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_store_error(error),
            ark_store::writer::TransactionFailure::Operation(error) => {
                DbError::from_application_error(error.to_string())
            }
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })
    })
}
