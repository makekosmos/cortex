use super::super::executor::actor::{ApplyRegistryEventsTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn apply_registry_events(
        &self,
        context: CommandContext,
        command: ApplyRegistryEventsCommand,
    ) -> Result<crate::replication_storage::RegistryPageApply, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_sync_peer_matches(&command.sender_device_id)?;
        if command.events.len() > 32 {
            return Err(DbError::Invalid("device_registry_page_bounds".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(ApplyRegistryEventsTask { command, reply });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}
