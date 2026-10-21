use super::*;
use std::time::Duration;

fn actor_path(directory: &crate::test_support::TestTempDir) -> std::path::PathBuf {
    directory.path().join("actor.db")
}

fn engine() -> CommandContext {
    CommandContext::engine()
}

#[derive(serde::Serialize)]
struct CommitFailure;

impl sealed::Sealed for CommitFailure {}

impl DbCommand for CommitFailure {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        _context: &CommandContext,
        capabilities: &mut ark_store::writer::WriteCapabilities<'_, '_>,
    ) -> Result<Self::Output, DbError> {
        capabilities
            .test_support()
            .trigger_deferred_commit_failure()
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok(())
    }
}

async fn open_failure_actor() -> (
    crate::test_support::TestTempDir,
    std::path::PathBuf,
    DbHandle,
    DbOwner,
) {
    let directory = crate::test_support::temp_dir();
    let path = actor_path(&directory);
    let path_text = path.to_str().expect("test database path is valid UTF-8");
    let (handle, owner) = DbHandle::open(path_text.to_owned(), "actor-test-device".into())
        .await
        .unwrap();
    (directory, path, handle, owner)
}

#[tokio::test]
async fn production_actor_recovers_interrupted_restore_before_ready() {
    let fixture = ark_store::test_support::InterruptedRestoreFixture::create().unwrap();
    let path = fixture.database_path().to_string_lossy().into_owned();

    let (_handle, mut owner) = DbHandle::open(path, "actor-test-device".into())
        .await
        .unwrap();
    let value = fixture.marker().unwrap();
    assert_eq!(value, "before");
    assert_eq!(
        fixture.inspect().unwrap(),
        ark_store::test_support::InterruptedRestoreState {
            intent: ark_store::test_support::RestoreIntentState::Missing,
            capsule_present: false,
        }
    );
    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_refuses_ready_when_restore_intent_is_corrupt() {
    let fixture = ark_store::test_support::InterruptedRestoreFixture::create().unwrap();
    fixture.corrupt_intent().unwrap();

    let opened = DbHandle::open(
        fixture.database_path().to_string_lossy().into_owned(),
        "actor-test-device".into(),
    )
    .await;

    assert!(matches!(opened, Err(DbError::RecoveryRequired)));
    assert_eq!(fixture.marker().unwrap(), "partial");
    assert_eq!(
        fixture.inspect().unwrap().intent,
        ark_store::test_support::RestoreIntentState::Corrupt
    );
}

#[tokio::test]
async fn production_actor_refuses_ready_when_restore_capsule_is_missing() {
    let fixture = ark_store::test_support::InterruptedRestoreFixture::create().unwrap();
    fixture.remove_capsule().unwrap();

    let opened = DbHandle::open(
        fixture.database_path().to_string_lossy().into_owned(),
        "actor-test-device".into(),
    )
    .await;

    assert!(matches!(opened, Err(DbError::RecoveryRequired)));
    assert_eq!(fixture.marker().unwrap(), "partial");
    let state = fixture.inspect().unwrap();
    assert_eq!(
        state.intent,
        ark_store::test_support::RestoreIntentState::Valid
    );
    assert!(!state.capsule_present);
}

fn failure_probe_state(
    path: &std::path::Path,
    operation_id: &str,
) -> ark_store::test_support::ActorFailureProbeState {
    ark_store::test_support::ProductionActorFailureProbe::inspect_path(path, operation_id).unwrap()
}

pub(in crate::db::command::tests::production_actor) async fn apply_failure_mutation(
    handle: &DbHandle,
    operation_id: &str,
    payload_bytes: i64,
) -> Result<super::super::super::OperationResult<String>, DbError> {
    handle
        .submit_with_operation_id(
            engine(),
            operation_id.to_owned(),
            "production_actor.failure_probe.v1",
            FailureMutation { payload_bytes },
        )
        .await
}

#[tokio::test]
async fn production_actor_sqlite_busy_rolls_back_and_accepts_a_retry() {
    let (_directory, path, handle, mut owner) = open_failure_actor().await;
    ark_store::test_support::ProductionActorFailureProbe::prepare_path(&path).unwrap();
    let competing_writer = ark_store::test_support::CompetingWriter::open(&path).unwrap();
    competing_writer.begin_immediate().unwrap();

    let failed = apply_failure_mutation(&handle, "busy-operation", 0).await;
    let is_busy = matches!(
        failed,
        Err(DbError::Storage(ref message)) if message.contains("database is locked")
    );
    assert!(
        is_busy,
        "competing writer should make the actor return SQLITE_BUSY, got {failed:?}"
    );
    assert_eq!(
        failure_probe_state(&path, "busy-operation"),
        ark_store::test_support::ActorFailureProbeState {
            mutations: 0,
            receipts: 0,
            deferred_children: 0,
        }
    );

    competing_writer.rollback().unwrap();
    let retry = apply_failure_mutation(&handle, "busy-operation", 0)
        .await
        .unwrap();
    assert_eq!(retry.value, "inserted:0");
    assert!(!retry.replayed);
    assert_eq!(
        failure_probe_state(&path, "busy-operation"),
        ark_store::test_support::ActorFailureProbeState {
            mutations: 1,
            receipts: 1,
            deferred_children: 0,
        }
    );

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_sqlite_full_rolls_back_and_fails_closed() {
    let (_directory, path, handle, mut owner) = open_failure_actor().await;
    ark_store::test_support::ProductionActorFailureProbe::prepare_path(&path).unwrap();
    let page_count =
        ark_store::test_support::ProductionActorFailureProbe::page_count_path(&path).unwrap();
    let limited_page_count = page_count + 1;
    assert_eq!(
        handle
            .submit_maintenance(
                CommandContext::engine_maintenance(),
                SetMaxPageCount {
                    page_count: limited_page_count,
                },
            )
            .await
            .unwrap(),
        limited_page_count
    );

    let failed = apply_failure_mutation(&handle, "full-operation", 1_048_576).await;
    let is_full = matches!(failed, Err(DbError::StorageUnhealthy(ref message))
        if message.contains("database or disk is full"));
    assert!(
        is_full,
        "page limit should make the actor return SQLITE_FULL, got {failed:?}"
    );
    assert_eq!(
        ark_store::test_support::ProductionActorFailureProbe::inspect_path(&path, "full-operation")
            .unwrap(),
        ark_store::test_support::ActorFailureProbeState {
            mutations: 0,
            receipts: 0,
            deferred_children: 0,
        }
    );

    let rejected = apply_failure_mutation(&handle, "later-operation", 0).await;
    assert!(rejected.is_err(), "failed actor accepted a later write");
    assert_eq!(
        ark_store::test_support::ProductionActorFailureProbe::inspect_path(
            &path,
            "later-operation"
        )
        .unwrap()
        .receipts,
        0
    );

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_commit_failure_rolls_back_receipt_and_fails_closed() {
    let (_directory, path, handle, mut owner) = open_failure_actor().await;
    ark_store::test_support::ProductionActorFailureProbe::prepare_path(&path).unwrap();

    let failed = handle
        .submit_with_operation_id(
            engine(),
            "commit-failure-operation".to_owned(),
            "production_actor.commit_failure.v1",
            CommitFailure,
        )
        .await;
    assert!(matches!(
        failed,
        Err(DbError::CommitFailed(ref message)) if message.contains("FOREIGN KEY constraint failed")
    ));
    tokio::time::timeout(Duration::from_secs(2), async {
        while handle.actor_ready_for_test() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actor stayed Ready after COMMIT failed");

    assert_eq!(
        ark_store::test_support::ProductionActorFailureProbe::inspect_path(
            &path,
            "commit-failure-operation"
        )
        .unwrap(),
        ark_store::test_support::ActorFailureProbeState {
            mutations: 0,
            receipts: 0,
            deferred_children: 0,
        }
    );

    let later = apply_failure_mutation(&handle, "after-commit-failure", 0).await;
    assert!(later.is_err(), "actor accepted a write after COMMIT failed");

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}
