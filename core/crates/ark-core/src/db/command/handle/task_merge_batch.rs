use super::super::executor::actor::{ApplyTaskMergeBatchTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn apply_task_merge_batch(
        &self,
        context: CommandContext,
        command: super::super::ApplyTaskMergeBatchCommand,
    ) -> Result<ark_application::sync_storage::TaskMergeBatchAck, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(ApplyTaskMergeBatchTask {
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
                spawn_test_apply_task_merge_batch(
                    Arc::clone(connection),
                    context,
                    command,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
fn spawn_test_apply_task_merge_batch(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    command: super::super::ApplyTaskMergeBatchCommand,
    admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<ark_application::sync_storage::TaskMergeBatchAck, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        context.require_sync_peer_matches(&command.peer_id)?;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::apply_inbound_task_merge_batch_on_connection(
            &mut connection,
            &command.peer_id,
            &command.local_device_id,
            command.expected_identity,
            command.batch,
        )
        .map_err(super::super::executor::actor::map_task_merge_batch_failure)
    })
}
