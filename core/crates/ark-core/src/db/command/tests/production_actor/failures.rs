use super::*;
use rusqlite::Connection;
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
            .map_err(DbError::from_sqlite)?;
        transaction
            .execute(
                "INSERT INTO production_actor_deferred_child(parent_id) VALUES (99)",
                [],
            )
            .map_err(DbError::from_sqlite)?;
        Ok(())
    }
}

struct RestoreRecoveryRequired;

impl sealed::Sealed for RestoreRecoveryRequired {}

impl DbMaintenanceCommand for RestoreRecoveryRequired {
    type Output = ();

    fn weight(&self) -> usize {
        1
    }

    fn execute(self, _connection: &mut Connection) -> Result<Self::Output, DbError> {
        Err(DbError::RecoveryRequired)
    }
}

struct RestorePostVerifyFailure {
    db_path: String,
    backup_id: String,
}

impl sealed::Sealed for RestorePostVerifyFailure {}

impl DbMaintenanceCommand for RestorePostVerifyFailure {
    type Output = ();

    fn weight(&self) -> usize {
        1
    }

    fn execute(self, connection: &mut Connection) -> Result<Self::Output, DbError> {
        let fail_verification = |_connection: &Connection, _fingerprint: &[String]| {
            Err("injected restore verification failure".to_string())
        };
        let rollback_succeeds = |live: &mut Connection, path: &std::path::Path| {
            live.restore(
                rusqlite::DatabaseName::Main,
                path,
                None::<fn(rusqlite::backup::Progress)>,
            )
            .map_err(|error| error.to_string())
        };
        crate::db::snapshot::restore_snapshot_impl(
            connection,
            &self.db_path,
            &self.backup_id,
            &fail_verification,
            &rollback_succeeds,
        )
        .map(|_| ())
        .map_err(DbError::RestoreFailed)
    }
}

async fn open_failure_actor() -> (tempfile::TempDir, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, owner) = DbHandle::open(actor_path(&directory), "actor-test-device".into())
        .await
        .unwrap();
    (directory, handle, owner)
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

async fn apply_failure_mutation(
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

#[tokio::test]
async fn production_actor_restore_recovery_required_fails_closed() {
    let (_directory, handle, mut owner) = open_failure_actor().await;

    let failed = handle
        .submit_maintenance(
            CommandContext::engine_maintenance(),
            RestoreRecoveryRequired,
        )
        .await;
    assert!(matches!(failed, Err(DbError::RecoveryRequired)));
    assert!(
        !handle.actor_ready_for_test(),
        "actor stayed Ready after restore rollback required recovery"
    );

    let later = apply_failure_mutation(&handle, "after-restore-recovery-required", 0).await;
    assert!(
        later.is_err(),
        "actor accepted a write after unresolved restore"
    );

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn production_actor_restore_failure_fails_closed_after_sqlite_rollback() {
    let (directory, handle, mut owner) = open_failure_actor().await;
    let db_path = actor_path(&directory);
    let backup_id = "ark.db.backup-restore-failure";
    let backup_path = crate::db::backups_dir(&db_path);
    std::fs::create_dir_all(&backup_path).unwrap();
    let snapshot_path = backup_path.join(backup_id);
    handle.with_connection(|connection| {
        connection
            .execute_batch(
                "CREATE TABLE restore_failure_probe(value TEXT NOT NULL);
                 INSERT INTO restore_failure_probe(value) VALUES ('pre-restore');",
            )
            .unwrap();
        crate::db::backup_to_file(
            connection,
            snapshot_path.to_str().expect("snapshot path must be UTF-8"),
        )
        .unwrap();
    });

    let failed = handle
        .submit_maintenance(
            CommandContext::engine_maintenance(),
            RestorePostVerifyFailure {
                db_path,
                backup_id: backup_id.to_string(),
            },
        )
        .await;
    assert!(matches!(failed, Err(DbError::RestoreFailed(_))));
    assert!(
        !handle.actor_ready_for_test(),
        "actor stayed Ready after a failed restore with a successful SQLite rollback"
    );
    assert!(matches!(
        apply_failure_mutation(&handle, "after-rolled-back-restore", 0).await,
        Err(DbError::Unavailable)
    ));
    let preserved = handle.with_connection(|connection| {
        connection
            .query_row("SELECT value FROM restore_failure_probe", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
    });
    assert_eq!(preserved, "pre-restore");

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}
