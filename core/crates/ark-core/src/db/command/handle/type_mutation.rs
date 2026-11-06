use super::super::executor::actor::{RegisterApplicationTypeTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_register_application_type(
        &self,
        context: CommandContext,
        registration: ark_domain::canonical_type::TypeRegistration,
    ) -> Result<Vec<ark_domain::sync_storage::ReplayNotice>, DbError> {
        let weight = validate(&context, "types.register_app.v1", &registration)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(RegisterApplicationTypeTask {
                registration,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}

pub(in crate::db::command::handle) fn validate(
    context: &CommandContext,
    command_type: &'static str,
    payload: &impl serde::Serialize,
) -> Result<usize, DbError> {
    if context.class != CommandClass::Interactive {
        return Err(DbError::Invalid("database command class mismatch".into()));
    }
    context.require_engine()?;
    let weight =
        serde_json::to_vec(&(command_type, payload)).map_or(usize::MAX, |bytes| bytes.len());
    if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
        Err(DbError::TooLarge)
    } else {
        Ok(weight)
    }
}
