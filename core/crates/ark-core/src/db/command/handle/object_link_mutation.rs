use super::super::executor::actor::{
    DeleteObjectLinkTask, ReplaceObjectLinkTask, UpsertObjectLinkTask, WriterTask,
};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_upsert_object_link(
        &self,
        context: CommandContext,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "object_link.upsert.v1", &object_link, &device_id)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(UpsertObjectLinkTask {
                    object_link,
                    device_id,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_link_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::upsert_object_link_on_connection(
                        connection,
                        object_link,
                        device_id,
                    )
                })
                .await
            }
        }
    }

    pub(crate) async fn submit_delete_object_link(
        &self,
        context: CommandContext,
        id: String,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "object_link.delete.v1", &id, &device_id)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(DeleteObjectLinkTask {
                    id,
                    device_id,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_link_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::delete_object_link_on_connection(
                        connection, id, device_id,
                    )
                })
                .await
            }
        }
    }

    pub(crate) async fn submit_replace_object_link(
        &self,
        context: CommandContext,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::ReplaceObjectLinkCommit, DbError> {
        let weight = validate(&context, "object_link.replace.v1", &object_link, &device_id)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(ReplaceObjectLinkTask {
                    object_link,
                    device_id,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_link_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::replace_object_link_on_connection(
                        connection,
                        object_link,
                        device_id,
                    )
                })
                .await
            }
        }
    }
}

fn validate(
    context: &CommandContext,
    command_type: &'static str,
    first: &impl serde::Serialize,
    second: &impl serde::Serialize,
) -> Result<usize, DbError> {
    if context.class != CommandClass::Interactive {
        return Err(DbError::Invalid("database command class mismatch".into()));
    }
    context.require_engine()?;
    let weight =
        serde_json::to_vec(&(command_type, first, second)).map_or(usize::MAX, |bytes| bytes.len());
    if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
        Err(DbError::TooLarge)
    } else {
        Ok(weight)
    }
}

#[cfg(debug_assertions)]
fn test_admission(
    handle: &DbHandle,
    context: &CommandContext,
    weight: usize,
) -> Result<executor::AdmissionPermit, DbError> {
    handle
        .admission
        .try_acquire(context.class, &context.source, weight)
        .map_err(map_admission_error)
}

#[cfg(debug_assertions)]
async fn spawn_test_link_mutation<T, F>(
    connection: Arc<Mutex<Connection>>,
    _admission: executor::AdmissionPermit,
    operation: F,
) -> Result<T, DbError>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T, ark_store::writer::TransactionFailure<String>>
        + Send
        + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        operation(&mut connection).map_err(map_storage_transaction_failure)
    })
    .await
    .map_err(join_error)?
}

#[cfg(debug_assertions)]
fn map_storage_transaction_failure(
    failure: ark_store::writer::TransactionFailure<String>,
) -> DbError {
    match failure {
        ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
        ark_store::writer::TransactionFailure::Operation(error) => {
            DbError::from_application_error(error)
        }
        ark_store::writer::TransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}

#[cfg(debug_assertions)]
fn join_error(error: tokio::task::JoinError) -> DbError {
    DbError::Storage(error.to_string())
}
