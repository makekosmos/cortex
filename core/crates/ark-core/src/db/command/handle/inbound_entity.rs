use super::super::executor::actor::{InboundEntityTask, WriterTask};
use super::*;
#[cfg(test)]
use ark_store::writer::SqliteWriter;

impl DbHandle {
    pub(crate) async fn apply_inbound_entity(
        &self,
        context: CommandContext,
        entity: crate::types::SyncEntity,
    ) -> Result<ark_store::writer::InboundEntityOutcome, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_system_sync()?;
        let weight = serde_json::to_vec(&entity).map_or(usize::MAX, |payload| payload.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(InboundEntityTask { entity, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let mut connection =
                            connection.lock().unwrap_or_else(|error| error.into_inner());
                        SqliteWriter::apply_inbound_entity_on_connection(&mut connection, entity)
                            .map_err(|failure| match failure {
                                ark_store::writer::TransactionFailure::Begin(error) => {
                                    DbError::from_store_error(error)
                                }
                                ark_store::writer::TransactionFailure::Operation(error) => {
                                    DbError::from_application_error(error)
                                }
                                ark_store::writer::TransactionFailure::Commit(error) => {
                                    DbError::CommitFailed(error.to_string())
                                }
                            })
                    }
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}
