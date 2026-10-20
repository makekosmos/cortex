use super::super::executor::actor::{
    DeleteObjectTypeTask, RegisterPackageTypesTask, UpsertObjectTypeTask, WriterTask,
};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_register_package_types(
        &self,
        context: CommandContext,
        registrations: Vec<ark_domain::canonical_type::TypeRegistration>,
    ) -> Result<Vec<ark_application::sync_storage::ReplayNotice>, DbError> {
        let weight = validate(&context, "types.register.v1", &registrations)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(RegisterPackageTypesTask {
                    registrations,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_type_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::register_package_types_on_connection(
                        connection,
                        registrations,
                    )
                })
                .await
            }
        }
    }

    pub(crate) async fn submit_upsert_object_type(
        &self,
        context: CommandContext,
        object_type: ark_domain::types::ObjectType,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "types.upsert.v1", &(&object_type, &device_id))?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(UpsertObjectTypeTask {
                    object_type,
                    device_id,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_type_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::upsert_object_type_on_connection(
                        connection,
                        object_type,
                        device_id,
                    )
                })
                .await
            }
        }
    }

    pub(crate) async fn submit_delete_object_type(
        &self,
        context: CommandContext,
        id: String,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "types.delete.v1", &(&id, &device_id))?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(DeleteObjectTypeTask {
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
                spawn_test_type_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::delete_object_type_on_connection(
                        connection, id, device_id,
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
    payload: &impl serde::Serialize,
) -> Result<usize, DbError> {
    if context.class != CommandClass::Interactive {
        return Err(DbError::Invalid("database command class mismatch".into()));
    }
    context.require_engine()?;
    let weight =
        serde_json::to_vec(&(command_type, payload)).map_or(usize::MAX, |bytes| bytes.len());
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
async fn spawn_test_type_mutation<T, F>(
    connection: Arc<Mutex<rusqlite::Connection>>,
    admission: executor::AdmissionPermit,
    operation: F,
) -> Result<T, DbError>
where
    T: Send + 'static,
    F: FnOnce(
            &mut rusqlite::Connection,
        ) -> Result<T, ark_store::writer::StoreTransactionFailure<String>>
        + Send
        + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        operation(&mut connection).map_err(map_store_failure)
    })
    .await
    .map_err(|error| DbError::Storage(error.to_string()))?
}

#[cfg(debug_assertions)]
fn map_store_failure(failure: ark_store::writer::StoreTransactionFailure<String>) -> DbError {
    use ark_store::writer::{StoreErrorKind, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => DbError::from_application_error(error),
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}
