use super::super::executor::actor::{LocalUsageWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_local_usage_write(
        &self,
        context: CommandContext,
        device_id: String,
        write: ark_application::usage::LocalUsageWrite,
    ) -> Result<Vec<ark_application::usage::UsageMutation>, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        #[derive(serde::Serialize)]
        struct UsageWrite<'a> {
            device_id: &'a str,
            write: &'a ark_application::usage::LocalUsageWrite,
        }
        let weight = serde_json::to_vec(&UsageWrite {
            device_id: &device_id,
            write: &write,
        })
        .map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(LocalUsageWriteTask {
                device_id,
                write,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
