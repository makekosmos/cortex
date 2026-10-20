use super::super::executor::actor::{ClearAllTask, DeleteTrashedTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_clear_all(&self, context: CommandContext) -> Result<(), DbError> {
        if context.class != CommandClass::Maintenance {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(ClearAllTask { reply });
                actor.admit(context.class, &context.source, 1, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, 1)
                    .map_err(map_admission_error)?;
                spawn_test_clear_all(Arc::clone(connection), admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn submit_delete_trashed(
        &self,
        context: CommandContext,
        device_id: String,
    ) -> Result<usize, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = device_id.len().max(1);
        if weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(DeleteTrashedTask { device_id, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_delete_trashed(Arc::clone(connection), device_id, admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_clear_all(
    connection: Arc<Mutex<Connection>>,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<(), DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::clear_all_on_connection(&mut connection)
            .map_err(map_storage_transaction_failure)
    })
}

#[cfg(debug_assertions)]
fn spawn_test_delete_trashed(
    connection: Arc<Mutex<Connection>>,
    device_id: String,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<usize, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::delete_trashed_on_connection(&mut connection, &device_id)
            .map_err(map_storage_transaction_failure)
    })
}

#[cfg(debug_assertions)]
fn map_storage_transaction_failure(
    failure: ark_store::writer::TransactionFailure<String>,
) -> DbError {
    match failure {
        ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
        ark_store::writer::TransactionFailure::Operation(error) => DbError::Storage(error),
        ark_store::writer::TransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}
