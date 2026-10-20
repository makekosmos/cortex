use super::super::commands::{
    AcceptPeerSyncPolicyCommand, SaveLocalSyncProfileCommand, SyncPolicyWrite,
    SyncPolicyWriteOutput,
};
use super::super::executor::actor::{SyncPolicyWriteTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn save_local_sync_profile(
        &self,
        context: CommandContext,
        command: SaveLocalSyncProfileCommand,
    ) -> Result<ark_application::sync_policy::SyncProfile, DbError> {
        match self
            .submit_sync_policy_write(context, SyncPolicyWrite::SaveLocal(command))
            .await?
        {
            SyncPolicyWriteOutput::LocalProfile(profile) => Ok(profile),
            SyncPolicyWriteOutput::AcceptedPeer(_) => {
                Err(DbError::Storage("sync policy write result mismatch".into()))
            }
        }
    }

    pub(crate) async fn accept_peer_sync_policy(
        &self,
        context: CommandContext,
        command: AcceptPeerSyncPolicyCommand,
    ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, DbError> {
        match self
            .submit_sync_policy_write(context, SyncPolicyWrite::AcceptPeer(command))
            .await?
        {
            SyncPolicyWriteOutput::AcceptedPeer(policy) => Ok(policy),
            SyncPolicyWriteOutput::LocalProfile(_) => {
                Err(DbError::Storage("sync policy write result mismatch".into()))
            }
        }
    }

    async fn submit_sync_policy_write(
        &self,
        context: CommandContext,
        write: SyncPolicyWrite,
    ) -> Result<SyncPolicyWriteOutput, DbError> {
        if context.class != write.class() {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = write.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(SyncPolicyWriteTask {
                    context: context.clone(),
                    write,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(debug_assertions)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_sync_policy_write(Arc::clone(connection), context, write, admission)
                    .await
                    .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(debug_assertions)]
fn spawn_test_sync_policy_write(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    write: SyncPolicyWrite,
    admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<SyncPolicyWriteOutput, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        write.execute_on_connection(&context, &mut connection)
    })
}
