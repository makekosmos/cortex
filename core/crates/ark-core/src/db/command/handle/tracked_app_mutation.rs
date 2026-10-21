use super::type_mutation::validate;
use super::*;
use executor::actor::TrackedAppMutationTask;

impl DbHandle {
    pub(crate) async fn submit_tracked_app_mutation(
        &self,
        context: CommandContext,
        mutation: ark_application::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<crate::types::SyncEntity, DbError> {
        let weight = validate(&context, "tracked_app.mutate.v1", &mutation)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(TrackedAppMutationTask { mutation, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
