use super::*;
#[cfg(any(test, feature = "test-support"))]
use ark_store::writer::SqliteWriter as StoreWriter;

impl DbHandle {
    pub(crate) async fn submit_signed_integration_frame(
        &self,
        context: CommandContext,
        command: ApplySignedIntegrationFrameCommand,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> =
                    Box::new(executor::actor::SignedIntegrationFrameTask {
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
                let (peer_id, request) = command.into_store_request(&context)?;
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let mut connection =
                            connection.lock().unwrap_or_else(|error| error.into_inner());
                        StoreWriter::apply_signed_integration_frame_on_connection(
                            &mut connection,
                            &peer_id,
                            request,
                        )
                        .map_err(|failure| match failure {
                            ark_store::writer::TransactionFailure::Begin(error) => {
                                DbError::from_store_error(error)
                            }
                            ark_store::writer::TransactionFailure::Operation(error) => {
                                map_signed_frame_operation_error(error)
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
