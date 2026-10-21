use super::super::executor::actor::{LocalCoreTaskWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_local_core_task_write(
        &self,
        context: CommandContext,
        operation_id: Option<String>,
        command: LocalCoreTaskWrite,
    ) -> Result<TaskWriteOperationResult, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        if operation_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        #[derive(serde::Serialize)]
        struct OperationWrite<'a> {
            operation_id: &'a str,
            command_type: &'static str,
            command: &'a LocalCoreTaskWrite,
        }
        let weight = operation_id.as_deref().map_or_else(
            || command.weight(),
            |operation_id| {
                serde_json::to_vec(&OperationWrite {
                    operation_id,
                    command_type: "object.upsert.v1",
                    command: &command,
                })
                .map_or(usize::MAX, |bytes| bytes.len())
            },
        );
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LocalCoreTaskWriteTask {
                context: context.clone(),
                command,
                operation_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
