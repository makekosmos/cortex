use super::super::executor::actor::{
    DeleteObjectLinkTask, ReplaceObjectLinkTask, UpsertObjectLinkTask, WriterTask,
};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_upsert_object_link(
        &self,
        context: CommandContext,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "object_link.upsert.v1", &object_link, &device_id)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(UpsertObjectLinkTask {
                object_link,
                device_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_delete_object_link(
        &self,
        context: CommandContext,
        id: String,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "object_link.delete.v1", &id, &device_id)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(DeleteObjectLinkTask {
                id,
                device_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_replace_object_link(
        &self,
        context: CommandContext,
        object_link: ark_domain::types::ObjectLink,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::ReplaceObjectLinkCommit, DbError> {
        let weight = validate(&context, "object_link.replace.v1", &object_link, &device_id)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(ReplaceObjectLinkTask {
                object_link,
                device_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}

fn validate(
    context: &CommandContext,
    command_type: &'static str,
    first: &impl serde::Serialize,
    second: &impl serde::Serialize,
) -> Result<usize, DbError> {
    if context.class != CommandClass::Interactive {
        return Err(DbError::Invalid("database command class mismatch".into()));
    }
    context.require_engine()?;
    let weight =
        serde_json::to_vec(&(command_type, first, second)).map_or(usize::MAX, |bytes| bytes.len());
    if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
        Err(DbError::TooLarge)
    } else {
        Ok(weight)
    }
}
