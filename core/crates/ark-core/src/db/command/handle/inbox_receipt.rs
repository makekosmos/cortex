use super::super::executor::actor::{InboxReceiptTask, WriterTask};
use super::*;
use ark_application::sync_storage::InboxReceiptWrite;

impl DbHandle {
    pub(crate) async fn receive_inbox_receipt(
        &self,
        context: CommandContext,
        write: InboxReceiptWrite,
    ) -> Result<crate::db::InboxState, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = write.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(InboxReceiptTask {
                context: context.clone(),
                write,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
