use super::super::executor::actor::{UsageSequenceMigrationTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn ensure_usage_sequence_migrated(
        &self,
        context: CommandContext,
        device_id: String,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        #[derive(serde::Serialize)]
        struct MigrationRequest<'a> {
            device_id: &'a str,
        }
        let weight = serde_json::to_vec(&MigrationRequest {
            device_id: &device_id,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(UsageSequenceMigrationTask {
                    context: context.clone(),
                    device_id,
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
                spawn_test_usage_sequence_migration(
                    Arc::clone(connection),
                    context,
                    device_id,
                    _admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_usage_sequence_migration(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    device_id: String,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<(), DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        context.require_engine()?;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::ensure_usage_sequence_migrated_on_connection(
            &mut connection,
            &device_id,
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
            ark_store::writer::TransactionFailure::Operation(error) => DbError::Storage(error),
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })
    })
}
