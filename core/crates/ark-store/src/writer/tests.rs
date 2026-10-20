use super::{SqliteWriter, TransactionFailure};
use rusqlite::Connection;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn immediate_transaction_commits_success_and_rolls_back_operation_error() {
    let connection = Connection::open_in_memory().expect("database opens");
    connection
        .execute_batch("CREATE TABLE values_table(value INTEGER NOT NULL)")
        .expect("test schema creates");
    let mut writer = SqliteWriter::from_connection(connection);

    writer
        .immediate(|transaction| {
            transaction.execute("INSERT INTO values_table VALUES (1)", [])?;
            Ok::<_, rusqlite::Error>(())
        })
        .expect("successful transaction commits");

    let failure = writer.immediate(|transaction| {
        transaction
            .execute("INSERT INTO values_table VALUES (2)", [])
            .map_err(|_| "test write fails")?;
        Err::<(), _>("injected operation failure")
    });
    assert!(matches!(
        failure,
        Err(TransactionFailure::Operation("injected operation failure"))
    ));

    let count = writer.with_connection_mut(|connection| {
        connection
            .query_row("SELECT COUNT(*) FROM values_table", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("committed row count reads")
    });
    assert_eq!(count, 1, "failed transaction leaves no partial write");
}

#[test]
fn deferred_commit_failure_rolls_back_and_releases_the_connection() {
    let connection = Connection::open_in_memory().expect("database opens");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE tx_parent(id INTEGER PRIMARY KEY);
             CREATE TABLE tx_child(
                 parent_id INTEGER REFERENCES tx_parent(id) DEFERRABLE INITIALLY DEFERRED
             );",
        )
        .expect("test schema creates");
    let mut writer = SqliteWriter::from_connection(connection);

    let failure = writer.immediate(|transaction| {
        transaction.execute("INSERT INTO tx_child(parent_id) VALUES (404)", [])?;
        Ok::<_, rusqlite::Error>(())
    });
    assert!(matches!(failure, Err(TransactionFailure::Commit(_))));

    let (autocommit, child_count) = writer.with_connection_mut(|connection| {
        let child_count = connection
            .query_row("SELECT COUNT(*) FROM tx_child", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("child count reads");
        (connection.is_autocommit(), child_count)
    });
    assert!(autocommit);
    assert_eq!(child_count, 0);

    writer
        .immediate(|transaction| {
            transaction.execute("INSERT INTO tx_parent(id) VALUES (1)", [])?;
            transaction.execute("INSERT INTO tx_child(parent_id) VALUES (1)", [])?;
            Ok::<_, rusqlite::Error>(())
        })
        .expect("connection accepts a valid write after failed commit");
}

#[test]
fn immediate_transaction_rolls_back_during_unwind() {
    let connection = Connection::open_in_memory().expect("database opens");
    connection
        .execute_batch("CREATE TABLE values_table(value INTEGER NOT NULL)")
        .expect("test schema creates");
    let mut writer = SqliteWriter::from_connection(connection);

    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = writer.immediate(|transaction| -> Result<(), rusqlite::Error> {
            transaction.execute("INSERT INTO values_table VALUES (3)", [])?;
            panic!("abort transaction closure");
        });
    }));
    assert!(panic.is_err());

    let (autocommit, count) = writer.with_connection_mut(|connection| {
        let count = connection
            .query_row("SELECT COUNT(*) FROM values_table", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("row count reads");
        (connection.is_autocommit(), count)
    });
    assert!(autocommit);
    assert_eq!(count, 0);
}
