use super::super::executor::actor::{AuthenticatedCoreTaskWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn submit_authenticated_core_task_write(
        &self,
        context: CommandContext,
        command: AuthenticatedCoreTaskWrite,
    ) -> Result<RemoteCoreTaskOutcome, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let peer_id = context.authenticated_peer_id()?.to_owned();
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        {
            let actor = &self.backend;
            let (reply, response) = tokio::sync::oneshot::channel();
            let task: Box<dyn WriterTask> = Box::new(AuthenticatedCoreTaskWriteTask {
                peer_id,
                command,
                reply,
            });
            actor.admit(context.class, &context.source, weight, task)?;
            response.await.unwrap_or(Err(DbError::OutcomeUnknown))
        }
    }
}

pub(in crate::db::command) fn map_authenticated_core_task_error(
    error: AuthenticatedCoreTaskWriteError,
) -> DbError {
    match error {
        AuthenticatedCoreTaskWriteError::MissingLocalDevice => {
            DbError::Permission("sync_policy_identity_missing_local_device".into())
        }
        AuthenticatedCoreTaskWriteError::StalePolicy => {
            DbError::Permission("stale_or_unnegotiated_sync_policy".into())
        }
        AuthenticatedCoreTaskWriteError::PeerNotAdmitted => {
            DbError::Permission("sync_peer_not_admitted".into())
        }
        AuthenticatedCoreTaskWriteError::PeerRemoved => {
            DbError::Permission("sync_peer_removed".into())
        }
        AuthenticatedCoreTaskWriteError::IncompleteRepresentation => {
            DbError::Unsupported("object_write_slice_requires_full_representation".into())
        }
        AuthenticatedCoreTaskWriteError::UnsupportedType => {
            DbError::Unsupported("object_write_slice_unsupported_type".into())
        }
        AuthenticatedCoreTaskWriteError::InvalidObject(error) => DbError::Invalid(error),
        AuthenticatedCoreTaskWriteError::Storage(error) => DbError::from_application_error(error),
    }
}
