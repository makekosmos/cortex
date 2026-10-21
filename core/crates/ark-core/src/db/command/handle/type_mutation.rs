use super::super::executor::actor::{
    DeleteObjectTypeTask, RegisterPackageTypesTask, UpsertObjectTypeTask, WriterTask,
};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_register_package_types(
        &self,
        context: CommandContext,
        registrations: Vec<ark_domain::canonical_type::TypeRegistration>,
    ) -> Result<Vec<ark_application::sync_storage::ReplayNotice>, DbError> {
        let weight = validate(&context, "types.register.v1", &registrations)?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(RegisterPackageTypesTask {
                registrations,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_upsert_object_type(
        &self,
        context: CommandContext,
        object_type: ark_domain::types::ObjectType,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "types.upsert.v1", &(&object_type, &device_id))?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(UpsertObjectTypeTask {
                object_type,
                device_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_delete_object_type(
        &self,
        context: CommandContext,
        id: String,
        device_id: Option<String>,
    ) -> Result<ark_application::type_mutation::TypeMutationCommit, DbError> {
        let weight = validate(&context, "types.delete.v1", &(&id, &device_id))?;
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(DeleteObjectTypeTask {
                id,
                device_id,
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
