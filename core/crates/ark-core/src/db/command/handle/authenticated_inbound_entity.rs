use super::super::executor::actor::{AuthenticatedInboundEntityTask, WriterTask};
use super::*;
use ark_application::sync_storage::InboxReceiptWrite;

impl DbHandle {
    pub(crate) async fn submit_authenticated_inbound_entity(
        &self,
        context: CommandContext,
        write: InboxReceiptWrite,
        entity: crate::types::SyncEntity,
    ) -> Result<ark_store::writer::AuthenticatedInboundEntityOutcome, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let peer_id = context.authenticated_peer_id()?.to_owned();
        if write.peer_device_id != peer_id {
            return Err(DbError::Permission(
                "authenticated_sync_peer_mismatch".into(),
            ));
        }
        let weight = write.weight().saturating_add(entity.data.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(AuthenticatedInboundEntityTask {
                peer_id,
                write,
                entity,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
