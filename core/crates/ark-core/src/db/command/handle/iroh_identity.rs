use super::type_mutation::{spawn_test_type_mutation, test_admission, validate};
use super::*;
use ark_store::writer::SqliteWriter;
use executor::actor::IrohIdentityKeyTask;

impl DbHandle {
    pub(crate) async fn submit_iroh_identity_key_candidate(
        &self,
        context: CommandContext,
        candidate_hex: String,
    ) -> Result<String, DbError> {
        let weight = validate(&context, "iroh.identity.persist.v1", &candidate_hex)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(IrohIdentityKeyTask {
                    candidate_hex,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_type_mutation(Arc::clone(connection), admission, move |connection| {
                    SqliteWriter::persist_iroh_secret_key_candidate_on_connection(
                        connection,
                        candidate_hex,
                    )
                })
                .await
            }
        }
    }
}
