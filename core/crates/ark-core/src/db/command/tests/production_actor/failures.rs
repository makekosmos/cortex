use super::*;
use std::time::Duration;

fn actor_path(directory: &tempfile::TempDir) -> String {
    directory
        .path()
        .join("actor.db")
        .to_string_lossy()
        .into_owned()
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
        transaction: &rusqlite::Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        transaction
            .execute(
                "INSERT INTO production_actor_failure_probe(payload) VALUES (zeroblob(0))",
                [],
            )
            .map_err(|error| DbError::Storage(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO production_actor_deferred_child(parent_id) VALUES (99)",
                [],
            )
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok(())
    }
}

async fn open_failure_actor() -> (tempfile::TempDir, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, owner) = DbHandle::open(actor_path(&directory), "actor-test-device".into())
        .await
        .unwrap();
    (directory, handle, owner)
}

#[tokio::test]
async fn production_actor_recovers_interrupted_restore_before_ready() {
    let fixture = ark_store::test_support::InterruptedRestoreFixture::create().unwrap();
    let path = fixture.database_path().to_string_lossy().into_owned();

    let (handle, mut owner) = DbHandle::open(path, "actor-test-device".into())
        .await
        .unwrap();
    let value = handle.with_connection(|connection| fixture.marker_on(connection).unwrap());
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

fn failure_probe_state(handle: &DbHandle, operation_id: &str) -> (i64, i64) {
    handle.with_connection(|connection| {
        let mutations = connection
            .query_row(
                "SELECT COUNT(*) FROM production_actor_failure_probe",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let receipts = connection
            .query_row(
                "SELECT COUNT(*) FROM ark_operation_receipts
                 WHERE operation_id = ?1",
                [operation_id],
                |row| row.get(0),
            )
            .unwrap();
        (mutations, receipts)
    })
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
    let (directory, handle, mut owner) = open_failure_actor().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE production_actor_failure_probe(payload BLOB NOT NULL);")
            .unwrap();
    });
    let competing_writer = Connection::open(actor_path(&directory)).unwrap();
    competing_writer.execute_batch("BEGIN IMMEDIATE;").unwrap();

    let failed = apply_failure_mutation(&handle, "busy-operation", 0).await;
    let is_busy = matches!(
        failed,
        Err(DbError::Storage(ref message)) if message.contains("database is locked")
    );
    assert!(
        is_busy,
        "competing writer should make the actor return SQLITE_BUSY, got {failed:?}"
    );
    assert_eq!(failure_probe_state(&handle, "busy-operation"), (0, 0));

    competing_writer.execute_batch("ROLLBACK;").unwrap();
    let retry = apply_failure_mutation(&handle, "busy-operation", 0)
        .await
        .unwrap();
    assert_eq!(retry.value, "inserted:0");
    assert!(!retry.replayed);
    assert_eq!(failure_probe_state(&handle, "busy-operation"), (1, 1));

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_sqlite_full_rolls_back_and_fails_closed() {
    let (directory, handle, mut owner) = open_failure_actor().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE production_actor_failure_probe(payload BLOB NOT NULL);")
            .unwrap();
    });
    let page_count = handle.with_connection(|connection| {
        connection
            .query_row("PRAGMA page_count", [], |row| row.get::<_, u32>(0))
            .unwrap()
    });
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
    let disk = Connection::open(actor_path(&directory)).unwrap();
    let mutations = disk
        .query_row(
            "SELECT COUNT(*) FROM production_actor_failure_probe",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    let receipts = disk
        .query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts WHERE operation_id = ?1",
            ["full-operation"],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!((mutations, receipts), (0, 0));

    let rejected = apply_failure_mutation(&handle, "later-operation", 0).await;
    assert!(rejected.is_err(), "failed actor accepted a later write");
    let later_receipts = disk
        .query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts WHERE operation_id = ?1",
            ["later-operation"],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(later_receipts, 0);

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_commit_failure_rolls_back_receipt_and_fails_closed() {
    let (directory, handle, mut owner) = open_failure_actor().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch(
                "CREATE TABLE production_actor_failure_probe(payload BLOB NOT NULL);
                 CREATE TABLE production_actor_parent(id INTEGER PRIMARY KEY);
                 CREATE TABLE production_actor_deferred_child(
                     parent_id INTEGER NOT NULL,
                     FOREIGN KEY(parent_id) REFERENCES production_actor_parent(id)
                         DEFERRABLE INITIALLY DEFERRED
                 );",
            )
            .unwrap();
    });

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

    let disk = Connection::open(actor_path(&directory)).unwrap();
    let mutation_rows = disk
        .query_row(
            "SELECT COUNT(*) FROM production_actor_failure_probe",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    let deferred_rows = disk
        .query_row(
            "SELECT COUNT(*) FROM production_actor_deferred_child",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    let receipt_rows = disk
        .query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts
             WHERE operation_id = 'commit-failure-operation'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!((mutation_rows, deferred_rows, receipt_rows), (0, 0, 0));

    let later = apply_failure_mutation(&handle, "after-commit-failure", 0).await;
    assert!(later.is_err(), "actor accepted a write after COMMIT failed");

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}
