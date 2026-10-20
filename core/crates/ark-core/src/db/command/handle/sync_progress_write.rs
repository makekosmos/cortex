use super::super::executor::actor::{SyncProgressWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn set_sync_progress(
        &self,
        context: CommandContext,
        write: ark_application::sync_storage::SyncProgressWrite,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = write.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(SyncProgressWriteTask {
                    context: context.clone(),
                    write,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let _admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_sync_progress_write(Arc::clone(connection), context, write, _admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_sync_progress_write(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    write: ark_application::sync_storage::SyncProgressWrite,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<(), DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        context.require_system_sync()?;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::set_sync_progress_on_connection(&mut connection, write)
            .map_err(map_sync_progress_failure)
    })
}

#[cfg(debug_assertions)]
fn map_sync_progress_failure(
    failure: ark_store::writer::TransactionFailure<
        ark_application::sync_storage::SyncProgressWriteError,
    >,
) -> DbError {
    match failure {
        ark_store::writer::TransactionFailure::Begin(error) => DbError::from_store_error(error),
        ark_store::writer::TransactionFailure::Operation(error) => match error {
            ark_application::sync_storage::SyncProgressWriteError::MissingLocalDevice => {
                DbError::Permission("sync_policy_identity_missing_local_device".into())
            }
            ark_application::sync_storage::SyncProgressWriteError::StalePolicy => {
                DbError::Permission("stale_or_unnegotiated_sync_policy".into())
            }
            ark_application::sync_storage::SyncProgressWriteError::PolicyStorage(error) => {
                DbError::from_application_error(error)
            }
            ark_application::sync_storage::SyncProgressWriteError::SyncValueStorage(error) => {
                DbError::Storage(error)
            }
        },
        ark_store::writer::TransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}
