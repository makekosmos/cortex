use super::super::executor::actor::{InboundEntityTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn apply_inbound_entity(
        &self,
        context: CommandContext,
        entity: crate::types::SyncEntity,
    ) -> Result<ark_store::writer::InboundEntityOutcome, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_system_sync()?;
        let weight = serde_json::to_vec(&entity).map_or(usize::MAX, |payload| payload.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(InboundEntityTask { entity, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
