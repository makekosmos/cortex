use super::super::executor::actor::{ExternalRefUpsertTask, WriterTask};
#[cfg(any(test, feature = "test-support"))]
use super::test_writer::spawn_test_store_writer;
use super::*;

impl DbHandle {
    pub(crate) async fn submit_external_ref_upsert(
        &self,
        context: CommandContext,
        request: ark_application::external_ref::ExternalRefUpsert,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = serde_json::to_vec(&request).map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(ExternalRefUpsertTask { request, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(any(test, feature = "test-support"))]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_store_writer(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::upsert_external_ref_on_connection(
                        connection, request,
                    )
                })
                .await
            }
        }
    }
}
