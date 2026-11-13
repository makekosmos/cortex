use super::super::executor::actor::{LocalSyncKeyValueWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn set_local_sync_key_value(
        &self,
        context: CommandContext,
        key: String,
        value: String,
    ) -> Result<(), DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        #[derive(serde::Serialize)]
        struct SyncKeyValueRequest<'a> {
            key: &'a str,
            value: &'a str,
        }
        let weight = serde_json::to_vec(&SyncKeyValueRequest {
            key: &key,
            value: &value,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LocalSyncKeyValueWriteTask {
                context: context.clone(),
                key,
                value,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
