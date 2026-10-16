use super::*;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct CounterWrite {
    amount: i64,
}

impl sealed::Sealed for CounterWrite {}

impl DbCommand for CounterWrite {
    type Output = String;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    fn execute(
        self,
        _context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        transaction
            .execute(
                "INSERT INTO operation_receipt_probe (amount) VALUES (?1)",
                [self.amount],
            )
            .map_err(|error| DbError::Storage(error.to_string()))?;
        let total: i64 = transaction
            .query_row(
                "SELECT SUM(amount) FROM operation_receipt_probe",
                [],
                |row| row.get(0),
            )
            .map_err(|error| DbError::Storage(error.to_string()))?;
        Ok(format!("total:{total}"))
    }
}

fn engine() -> CommandContext {
    CommandContext::from_trusted_principal(CommandClass::Interactive, CommandSource::Engine)
}

async fn open_receipt_fixture() -> (tempfile::TempDir, DbHandle, DbOwner) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join("receipts.db")
        .to_string_lossy()
        .into_owned();
    let (handle, owner) = DbHandle::open(path, "receipt-test-device".into())
        .await
        .unwrap();
    (directory, handle, owner)
}

async fn apply(
    handle: &DbHandle,
    context: CommandContext,
    operation_id: &str,
    amount: i64,
) -> Result<super::super::OperationResult<String>, DbError> {
    handle
        .submit_with_operation_id(
            context,
            operation_id.to_owned(),
            "counter.write.v1",
            CounterWrite { amount },
        )
        .await
}

fn probe_total(handle: &DbHandle) -> i64 {
    handle.with_connection(|connection| {
        connection
            .query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM operation_receipt_probe",
                [],
                |row| row.get(0),
            )
            .unwrap()
    })
}

#[tokio::test]
async fn concurrent_duplicate_operations_commit_once_and_return_the_stored_result() {
    let (_directory, handle, mut owner) = open_receipt_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE operation_receipt_probe (amount INTEGER NOT NULL);")
            .unwrap();
    });

    let (left, right) = tokio::join!(
        apply(&handle, engine(), "same-operation", 7),
        apply(&handle, engine(), "same-operation", 7),
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.value, "total:7");
    assert_eq!(right.value, "total:7");
    assert_ne!(left.replayed, right.replayed);
    assert_eq!(probe_total(&handle), 7);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn response_loss_retry_replays_the_committed_result() {
    let (_directory, handle, mut owner) = open_receipt_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE operation_receipt_probe (amount INTEGER NOT NULL);")
            .unwrap();
    });

    let first = apply(&handle, engine(), "lost-response", 11).await.unwrap();
    let retry = apply(&handle, engine(), "lost-response", 11).await.unwrap();
    assert_eq!(first.value, "total:11");
    assert!(!first.replayed);
    assert_eq!(retry.value, "total:11");
    assert!(retry.replayed);
    assert_eq!(probe_total(&handle), 11);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn operation_id_rejects_a_different_canonical_payload() {
    let (_directory, handle, mut owner) = open_receipt_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE operation_receipt_probe (amount INTEGER NOT NULL);")
            .unwrap();
    });

    apply(&handle, engine(), "payload-conflict", 2)
        .await
        .unwrap();
    assert!(matches!(
        apply(&handle, engine(), "payload-conflict", 3).await,
        Err(DbError::Conflict(_))
    ));
    assert_eq!(probe_total(&handle), 2);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn operation_id_is_scoped_to_the_trusted_command_principal() {
    let (_directory, handle, mut owner) = open_receipt_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE operation_receipt_probe (amount INTEGER NOT NULL);")
            .unwrap();
    });
    let package_a = CommandContext::from_trusted_principal(
        CommandClass::Interactive,
        CommandSource::ApplicationPackage("package-a".into()),
    );
    let package_b = CommandContext::from_trusted_principal(
        CommandClass::Interactive,
        CommandSource::ApplicationPackage("package-b".into()),
    );

    let first = apply(&handle, package_a.clone(), "scoped-operation", 4)
        .await
        .unwrap();
    let second = apply(&handle, package_b, "scoped-operation", 9)
        .await
        .unwrap();
    let replay = apply(&handle, package_a, "scoped-operation", 4)
        .await
        .unwrap();
    assert_eq!(first.value, "total:4");
    assert_eq!(second.value, "total:13");
    assert_eq!(replay.value, "total:4");
    assert!(!first.replayed && !second.replayed && replay.replayed);
    assert_eq!(probe_total(&handle), 13);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn receipt_insert_failure_rolls_back_the_mutation_and_allows_retry() {
    let (_directory, handle, mut owner) = open_receipt_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch(
                "CREATE TABLE operation_receipt_probe (amount INTEGER NOT NULL);
                 CREATE TRIGGER reject_operation_receipt
                 BEFORE INSERT ON ark_operation_receipts
                 BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END;",
            )
            .unwrap();
    });

    assert!(apply(&handle, engine(), "rollback-operation", 5)
        .await
        .is_err());
    assert_eq!(probe_total(&handle), 0);
    handle.with_connection(|connection| {
        connection
            .execute_batch("DROP TRIGGER reject_operation_receipt;")
            .unwrap();
    });
    let retry = apply(&handle, engine(), "rollback-operation", 5)
        .await
        .unwrap();
    assert_eq!(retry.value, "total:5");
    assert!(!retry.replayed);
    assert_eq!(probe_total(&handle), 5);

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn operation_receipt_replays_after_database_reopen() {
    let (_directory, handle, mut owner) = open_receipt_fixture().await;
    handle.with_connection(|connection| {
        connection
            .execute_batch("CREATE TABLE operation_receipt_probe (amount INTEGER NOT NULL);")
            .unwrap();
    });
    let path = _directory
        .path()
        .join("receipts.db")
        .to_string_lossy()
        .into_owned();
    let first = apply(&handle, engine(), "reopen-operation", 13)
        .await
        .unwrap();
    assert_eq!(first.value, "total:13");
    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
    drop(handle);

    let (reopened, mut reopened_owner) = DbHandle::open(path, "actor-test-device".into())
        .await
        .unwrap();
    let replay = apply(&reopened, engine(), "reopen-operation", 13)
        .await
        .unwrap();
    assert_eq!(replay.value, "total:13");
    assert!(replay.replayed);
    assert_eq!(probe_total(&reopened), 13);
    reopened_owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
