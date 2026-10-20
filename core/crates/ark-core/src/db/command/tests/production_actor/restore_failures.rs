use super::failures::apply_failure_mutation;
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

async fn open_failure_actor() -> (tempfile::TempDir, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, owner) = DbHandle::open(actor_path(&directory), "actor-test-device".into())
        .await
        .unwrap();
    (directory, handle, owner)
}

struct RestoreRecoveryRequired;

impl sealed::Sealed for RestoreRecoveryRequired {}

impl DbMaintenanceCommand for RestoreRecoveryRequired {
    type Output = ();

    fn weight(&self) -> usize {
        1
    }

    fn execute(
        self,
        _writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<Self::Output, DbError> {
        Err(DbError::RecoveryRequired)
    }

    fn execute_on_connection(self, _connection: &mut Connection) -> Result<Self::Output, DbError> {
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

    fn execute(
        self,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<Self::Output, DbError> {
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
        writer
            .restore_snapshot_with_hooks(
                &self.db_path,
                &self.backup_id,
                fail_verification,
                rollback_succeeds,
            )
            .map(|_| ())
            .map_err(DbError::RestoreFailed)
    }

    fn execute_on_connection(self, connection: &mut Connection) -> Result<Self::Output, DbError> {
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
        ark_store::snapshot::restore_snapshot_with_hooks(
            connection,
            &self.db_path,
            &self.backup_id,
            fail_verification,
            rollback_succeeds,
        )
        .map(|_| ())
        .map_err(DbError::RestoreFailed)
    }
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
    ark_store::test_support::ProductionActorFailureProbe::create_restore_snapshot_fixture(
        std::path::Path::new(&db_path),
        &snapshot_path,
    )
    .unwrap();

    let failed = handle
        .submit_maintenance(
            CommandContext::engine_maintenance(),
            RestorePostVerifyFailure {
                db_path: db_path.clone(),
                backup_id: backup_id.to_string(),
            },
        )
        .await;
    assert!(matches!(failed, Err(DbError::RestoreFailed(_))));
    assert!(
        !handle.actor_ready_for_test(),
        "actor stayed Ready after a failed restore with a successful SQLite rollback"
    );
    assert!(
        apply_failure_mutation(&handle, "after-rolled-back-restore", 0)
            .await
            .is_err(),
        "actor accepted a write after a failed restore"
    );
    let preserved = ark_store::test_support::ProductionActorFailureProbe::restore_probe_value(
        std::path::Path::new(&db_path),
    )
    .unwrap();
    assert_eq!(preserved, "pre-restore");

    owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
}
