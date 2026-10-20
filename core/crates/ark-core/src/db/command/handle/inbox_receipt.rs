#[cfg(debug_assertions)]
use super::super::executor::actor::map_inbox_receipt_error;
use super::super::executor::actor::{InboxReceiptTask, WriterTask};
use super::*;
use ark_application::sync_storage::InboxReceiptWrite;
#[cfg(debug_assertions)]
use ark_store::writer::SqliteWriter;

impl DbHandle {
    pub(crate) async fn receive_inbox_receipt(
        &self,
        context: CommandContext,
        write: InboxReceiptWrite,
    ) -> Result<crate::db::InboxState, DbError> {
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
                let task: Box<dyn WriterTask> = Box::new(InboxReceiptTask {
                    context: context.clone(),
                    write,
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
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let result = context
                            .require_sync_peer_matches(&write.peer_device_id)
                            .and_then(|()| {
                                let mut connection =
                                    connection.lock().unwrap_or_else(|error| error.into_inner());
                                SqliteWriter::receive_inbox_receipt_on_connection(
                                    &mut connection,
                                    write,
                                )
                                .map_err(|failure| match failure {
                                    ark_store::writer::TransactionFailure::Begin(error) => {
                                        DbError::from_sqlite(error)
                                    }
                                    ark_store::writer::TransactionFailure::Operation(error) => {
                                        map_inbox_receipt_error(error)
                                    }
                                    ark_store::writer::TransactionFailure::Commit(error) => {
                                        DbError::CommitFailed(error.to_string())
                                    }
                                })
                            });
                        result
                    }
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}
