use super::super::executor::actor::{AuthenticatedInboundEntityTask, WriterTask};
use super::*;
use ark_application::sync_storage::InboxReceiptWrite;

impl DbHandle {
    pub(crate) async fn submit_authenticated_inbound_entity(
        &self,
        context: CommandContext,
        write: InboxReceiptWrite,
        entity: crate::types::SyncEntity,
    ) -> Result<ark_store::writer::AuthenticatedInboundEntityOutcome, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let peer_id = context.authenticated_peer_id()?.to_owned();
        if write.peer_device_id != peer_id {
            return Err(DbError::Permission(
                "authenticated_sync_peer_mismatch".into(),
            ));
        }
        let weight = write.weight().saturating_add(entity.data.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(AuthenticatedInboundEntityTask {
                    peer_id,
                    write,
                    entity,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_authenticated_inbound_entity(
                    Arc::clone(connection),
                    write,
                    entity,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_authenticated_inbound_entity(
    connection: Arc<Mutex<Connection>>,
    write: InboxReceiptWrite,
    entity: crate::types::SyncEntity,
    _admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<ark_store::writer::AuthenticatedInboundEntityOutcome, DbError>>
{
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        ark_store::writer::SqliteWriter::apply_authenticated_inbound_entity_on_connection(
            &mut connection,
            write,
            entity,
        )
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
            ark_store::writer::TransactionFailure::Operation(error) => match error {
                ark_store::writer::AuthenticatedInboundEntityError::Mutation(error) => {
                    DbError::from_application_error(error)
                }
                ark_store::writer::AuthenticatedInboundEntityError::Receipt(error) => {
                    super::super::executor::actor::map_inbox_receipt_error(error)
                }
            },
            ark_store::writer::TransactionFailure::Commit(error) => {
                DbError::CommitFailed(error.to_string())
            }
        })
    })
}
