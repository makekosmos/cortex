use super::super::executor::actor::{LocalSyncKeyValueWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn set_local_sync_key_value(
        &self,
        context: CommandContext,
        key: String,
        value: String,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        #[derive(serde::Serialize)]
        struct SyncKeyValueRequest<'a> {
            key: &'a str,
            value: &'a str,
        }
        let weight = serde_json::to_vec(&SyncKeyValueRequest {
            key: &key,
            value: &value,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(LocalSyncKeyValueWriteTask {
                    context: context.clone(),
                    key,
                    value,
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
                spawn_test_local_sync_key_value_write(
                    Arc::clone(connection),
                    context,
                    key,
                    value,
                    _admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_local_sync_key_value_write(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    key: String,
    value: String,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<(), DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        context.require_engine()?;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::set_local_sync_key_value_on_connection(
            &mut connection,
            &key,
            &value,
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_store_error(error),
            ark_store::writer::TransactionFailure::Operation(error) => DbError::Storage(error),
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })
    })
}
