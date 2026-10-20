use super::type_mutation::validate;
#[cfg(debug_assertions)]
use super::type_mutation::{spawn_test_type_mutation, test_admission};
use super::*;
use executor::actor::TrackedAppMutationTask;

impl DbHandle {
    pub(crate) async fn submit_tracked_app_mutation(
        &self,
        context: CommandContext,
        mutation: ark_application::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<crate::types::SyncEntity, DbError> {
        let weight = validate(&context, "tracked_app.mutate.v1", &mutation)?;
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> =
                    Box::new(TrackedAppMutationTask { mutation, reply });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = test_admission(self, &context, weight)?;
                spawn_test_type_mutation(Arc::clone(connection), admission, move |connection| {
                    ark_store::writer::SqliteWriter::apply_tracked_app_mutation_on_connection(
                        connection, mutation,
                    )
                })
                .await
            }
        }
    }
}
