use super::super::commands::CompactUsageSyncLogCommand;
use super::super::executor::actor::{UsageCompactionTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn compact_usage_sync_log(
        &self,
        context: CommandContext,
        command: CompactUsageSyncLogCommand,
    ) -> Result<usize, DbError> {
        if context.class != CommandClass::Maintenance {
            return Err(DbError::Invalid(
                "maintenance command class required".into(),
            ));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(UsageCompactionTask {
                    context: context.clone(),
                    command,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(any(test, feature = "test-support"))]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_usage_compaction(Arc::clone(connection), context, command, admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
fn spawn_test_usage_compaction(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    command: CompactUsageSyncLogCommand,
    admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<usize, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        command.execute_on_connection(&context, &mut connection)
    })
}
