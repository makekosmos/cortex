use super::super::executor::actor::{UsageSequenceMigrationTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn ensure_usage_sequence_migrated(
        &self,
        context: CommandContext,
        device_id: String,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        #[derive(serde::Serialize)]
        struct MigrationRequest<'a> {
            device_id: &'a str,
        }
        let weight = serde_json::to_vec(&MigrationRequest {
            device_id: &device_id,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(UsageSequenceMigrationTask {
                context: context.clone(),
                device_id,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
