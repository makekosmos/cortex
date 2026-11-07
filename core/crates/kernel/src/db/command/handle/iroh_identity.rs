use super::type_mutation::validate;
use super::*;
use executor::actor::IrohIdentityKeyTask;

impl DbHandle {
    pub(crate) async fn submit_iroh_identity_key_candidate(
        &self,
        context: CommandContext,
        candidate_hex: String,
    ) -> Result<String, DbError> {
        let weight = validate(&context, "iroh.identity.persist.v1", &candidate_hex)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(IrohIdentityKeyTask {
                candidate_hex,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
