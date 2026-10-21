use super::super::executor::actor::{
    CanonicalGameUpsertTask, LocalBookCoverTask, LocalObjectDeleteTask, LocalObjectUpsertTask,
    WriterTask,
};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_canonical_game_upsert(
        &self,
        context: CommandContext,
        command: ark_application::canonical_game::GameUpsertCommand,
        fallback_device_id: String,
    ) -> Result<ark_application::canonical_game::GameMutationResult, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        #[derive(serde::Serialize)]
        struct GameCommand<'a> {
            command: &'a ark_application::canonical_game::GameUpsertCommand,
            fallback_device_id: &'a str,
        }
        let weight = serde_json::to_vec(&GameCommand {
            command: &command,
            fallback_device_id: &fallback_device_id,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(CanonicalGameUpsertTask {
                command,
                fallback_device_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_local_book_cover(
        &self,
        context: CommandContext,
        write: ark_application::book_cover::LocalBookCoverWrite,
    ) -> Result<bool, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        #[derive(serde::Serialize)]
        struct BookCoverCommand<'a> {
            write: &'a ark_application::book_cover::LocalBookCoverWrite,
        }
        let weight = serde_json::to_vec(&BookCoverCommand { write: &write })
            .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }

        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LocalBookCoverTask { write, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }

    pub(crate) async fn submit_local_object_upsert(
        &self,
        context: CommandContext,
        write: ark_application::object_mutation::LocalObjectUpsert,
        operation_id: Option<String>,
    ) -> Result<
        super::super::operation_receipt::OperationResult<
            ark_application::object_mutation::ObjectUpsertCommit,
        >,
        DbError,
    > {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        if operation_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        #[derive(serde::Serialize)]
        struct UpsertCommand<'a> {
            write: &'a ark_application::object_mutation::LocalObjectUpsert,
        }
        #[derive(serde::Serialize)]
        struct ReceiptCommand<'a> {
            operation_id: &'a Option<String>,
            command_type: &'static str,
            command: UpsertCommand<'a>,
        }
        let weight = serde_json::to_vec(&ReceiptCommand {
            operation_id: &operation_id,
            command_type: "object.upsert.v1",
            command: UpsertCommand { write: &write },
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }

        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LocalObjectUpsertTask {
                write,
                principal: context.operation_principal(),
                operation_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            let outcome = response.await.unwrap_or(Err(DbError::OutcomeUnknown))?;
            Ok(super::super::operation_receipt::OperationResult {
                value: outcome.value,
                replayed: outcome.replayed,
            })
        }
    }

    pub(crate) async fn submit_local_object_delete(
        &self,
        context: CommandContext,
        write: ark_application::object_mutation::LocalObjectDelete,
        operation_id: Option<String>,
    ) -> Result<
        super::super::operation_receipt::OperationResult<
            ark_application::object_mutation::ObjectDeleteCommit,
        >,
        DbError,
    > {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        if operation_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DbError::Invalid(
                "operationId must contain 1 to 256 bytes".into(),
            ));
        }
        #[derive(serde::Serialize)]
        struct DeleteCommand<'a> {
            write: &'a ark_application::object_mutation::LocalObjectDelete,
        }
        #[derive(serde::Serialize)]
        struct ReceiptCommand<'a> {
            operation_id: &'a Option<String>,
            command_type: &'static str,
            command: DeleteCommand<'a>,
        }
        let weight = serde_json::to_vec(&ReceiptCommand {
            operation_id: &operation_id,
            command_type: "object.delete.v1",
            command: DeleteCommand { write: &write },
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }

        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LocalObjectDeleteTask {
                write,
                principal: context.operation_principal(),
                operation_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            let (value, replayed) = response.await.unwrap_or(Err(DbError::OutcomeUnknown))?;
            Ok(super::super::operation_receipt::OperationResult { value, replayed })
        }
    }
}
