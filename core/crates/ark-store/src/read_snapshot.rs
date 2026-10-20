use crate::writer::{store_error_from_sqlite, StoreError};
use crate::ReadCapabilities;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::fmt;

pub struct SqliteReader {
    connection: Connection,
}

#[derive(Debug)]
pub enum ReadSnapshotFailure<E> {
    Begin(StoreError),
    Operation(E),
    Commit(StoreError),
}

impl<E: fmt::Display> fmt::Display for ReadSnapshotFailure<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Begin(error) => write!(formatter, "read snapshot begin failed: {error}"),
            Self::Operation(error) => write!(formatter, "read operation failed: {error}"),
            Self::Commit(error) => write!(formatter, "read snapshot commit failed: {error}"),
        }
    }
}

impl SqliteReader {
    pub fn open_read_only(path: &str) -> Result<Self, StoreError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(store_error_from_sqlite)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(store_error_from_sqlite)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(store_error_from_sqlite)?;
        Ok(Self { connection })
    }

    pub fn with_read_snapshot<T, E>(
        &mut self,
        operation: impl for<'snapshot> FnOnce(ReadCapabilities<'snapshot>) -> Result<T, E>,
    ) -> Result<T, ReadSnapshotFailure<E>> {
        with_read_snapshot_on_connection(&mut self.connection, operation)
    }
}

fn with_read_snapshot_on_connection<T, E>(
    connection: &mut Connection,
    operation: impl for<'snapshot> FnOnce(ReadCapabilities<'snapshot>) -> Result<T, E>,
) -> Result<T, ReadSnapshotFailure<E>> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(store_error_from_sqlite)
        .map_err(ReadSnapshotFailure::Begin)?;
    let result =
        operation(ReadCapabilities::new(&transaction)).map_err(ReadSnapshotFailure::Operation)?;
    transaction
        .commit()
        .map_err(store_error_from_sqlite)
        .map_err(ReadSnapshotFailure::Commit)?;
    Ok(result)
}

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::{with_read_snapshot_on_connection, ReadSnapshotFailure};
    use crate::ReadCapabilities;
    use rusqlite::Connection;

    pub fn with_read_snapshot<T, E>(
        connection: &mut Connection,
        operation: impl for<'snapshot> FnOnce(ReadCapabilities<'snapshot>) -> Result<T, E>,
    ) -> Result<T, ReadSnapshotFailure<E>> {
        with_read_snapshot_on_connection(connection, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadSnapshotFailure, SqliteReader};
    use crate::objects::upsert_object_compat;
    use ark_domain::types::ArkObject;
    use serde_json::json;
    use std::sync::mpsc;

    fn object(title: &str) -> ArkObject {
        ArkObject {
            id: "object".into(),
            type_id: "com.kosmos.note".into(),
            type_version: "1.0.0".into(),
            title: title.into(),
            content_json: json!({"type":"doc","content":[]}),
            props_json: json!({"description":null,"extensions":{}}),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deleted_at: None,
        }
    }

    #[test]
    fn reads_share_one_snapshot_while_a_writer_commits() {
        let directory = tempfile::tempdir().expect("temporary directory creates");
        let path = directory.path().join("ark.db");
        let path = path.to_str().expect("temporary path is UTF-8");
        let connection = crate::open::open_db(path).expect("database opens");
        crate::open::init_schema(&connection).expect("database schema initializes");
        upsert_object_compat(&connection, &object("before")).expect("initial object writes");

        let (read_tx, read_rx) = mpsc::channel();
        let (write_tx, write_rx) = mpsc::channel();
        let writer_path = path.to_owned();
        let writer = std::thread::spawn(move || {
            read_rx.recv().expect("reader starts writer");
            let connection = crate::open::open_db(&writer_path).expect("writer opens database");
            upsert_object_compat(&connection, &object("after")).expect("updated object writes");
            write_tx.send(()).expect("writer reports commit");
        });

        let mut reader = SqliteReader::open_read_only(path).expect("reader opens database");
        reader
            .with_read_snapshot(|capabilities| {
                assert_eq!(
                    capabilities
                        .objects()
                        .get("object")?
                        .expect("object exists")
                        .title,
                    "before"
                );
                read_tx.send(()).expect("writer starts");
                write_rx.recv().expect("writer commits update");
                assert_eq!(
                    capabilities
                        .objects()
                        .get("object")?
                        .expect("object exists")
                        .title,
                    "before"
                );
                Ok::<_, String>(())
            })
            .expect("read transaction commits");
        writer.join().expect("writer thread joins");

        reader
            .with_read_snapshot(|capabilities| {
                assert_eq!(
                    capabilities
                        .objects()
                        .get("object")?
                        .expect("object exists")
                        .title,
                    "after"
                );
                Ok::<_, String>(())
            })
            .expect("next snapshot observes committed update");
    }

    #[test]
    fn operation_failure_is_preserved_as_the_operation_stage() {
        let directory = tempfile::tempdir().expect("temporary directory creates");
        let path = directory.path().join("ark.db");
        let path = path.to_str().expect("temporary path is UTF-8");
        let connection = crate::open::open_db(path).expect("database opens");
        crate::open::init_schema(&connection).expect("database schema initializes");
        drop(connection);
        let mut reader = SqliteReader::open_read_only(path).expect("reader opens database");
        let result = reader.with_read_snapshot(|_| Err::<(), _>("expected failure"));

        assert!(matches!(
            result,
            Err(ReadSnapshotFailure::Operation("expected failure"))
        ));
    }
}
