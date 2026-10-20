use super::super::executor::actor::{CommittedFeedProgressTask, WriterTask};
use super::*;

impl DbHandle {
    pub(crate) async fn apply_committed_feed_progress(
        &self,
        context: CommandContext,
        command: CommittedFeedProgressCommand,
    ) -> Result<CommittedFeedProgress, DbError> {
        if context.class != CommandClass::SyncLive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        let weight = command.weight();
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(CommittedFeedProgressTask {
                    context: context.clone(),
                    command,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(any(test, feature = "test-support"))]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                spawn_test_committed_feed_progress(
                    Arc::clone(connection),
                    context,
                    command,
                    admission,
                )
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
fn spawn_test_committed_feed_progress(
    connection: Arc<Mutex<Connection>>,
    context: CommandContext,
    command: CommittedFeedProgressCommand,
    admission: executor::AdmissionPermit,
) -> tokio::task::JoinHandle<Result<CommittedFeedProgress, DbError>> {
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        command.execute_on_connection(&context, &mut connection)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_application::sync_storage::{
        OutboundProgressMutation, OutboundProgressPolicy, OutboundProgressWrite,
    };
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn actor_checks_peer_before_committing_progress() {
        let directory = tempfile::tempdir().expect("temporary directory opens");
        let path = directory.path().join("ark.db");
        let (handle, mut owner) = DbHandle::open(
            path.to_str().expect("database path is UTF-8").to_owned(),
            "local-device".into(),
        )
        .await
        .expect("actor database opens");

        let make_command = || {
            CommittedFeedProgressCommand::new(OutboundProgressWrite::new(
                "peer-a".into(),
                OutboundProgressPolicy::Generation(1),
                OutboundProgressMutation::Begin,
            ))
        };
        let denied = handle
            .apply_committed_feed_progress(
                CommandContext::sync_peer("peer-b".into(), CommandClass::SyncLive),
                make_command(),
            )
            .await;
        assert!(matches!(denied, Err(DbError::Permission(_))));

        let committed = handle
            .apply_committed_feed_progress(
                CommandContext::sync_peer("peer-a".into(), CommandClass::SyncLive),
                make_command(),
            )
            .await
            .expect("matching peer can begin its committed-feed progress");
        let CommittedFeedProgress::Outbound(progress) = committed;
        assert_eq!(progress.policy_generation, 1);
        assert!(!progress.initial_snapshot.is_complete());

        owner
            .shutdown(Instant::now() + Duration::from_secs(2))
            .await
            .expect("actor shuts down cleanly");
    }
}
