use super::super::executor::actor::{ApplyRegistryEventsTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn apply_registry_events(
        &self,
        context: CommandContext,
        command: ApplyRegistryEventsCommand,
    ) -> Result<crate::replication_storage::RegistryPageApply, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_sync_peer_matches(&command.sender_device_id)?;
        if command.events.len() > 32 {
            return Err(DbError::Invalid("device_registry_page_bounds".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> =
                    Box::new(ApplyRegistryEventsTask { command, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(any(test, feature = "test-support"))]
            HandleBackend::Test(connection) => {
                let connection = Arc::clone(connection);
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                tokio::task::spawn_blocking(move || {
                    let _admission = admission;
                    let mut connection =
                        connection.lock().unwrap_or_else(|error| error.into_inner());
                    ark_store::writer::SqliteWriter::apply_device_registry_events_on_connection(
                        &mut connection,
                        &command.sender_device_id,
                        &command.events,
                    )
                    .map_err(|failure| match failure {
                        ark_store::writer::StoreTransactionFailure::Begin(error) => {
                            DbError::Storage(error.to_string())
                        }
                        ark_store::writer::StoreTransactionFailure::Operation(error) => {
                            super::super::executor::actor::map_registry_event_error(error)
                        }
                        ark_store::writer::StoreTransactionFailure::Commit(error) => {
                            DbError::CommitFailed(error.to_string())
                        }
                    })
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}
