use std::path::Path;

use rusqlite::{Connection, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectWriteRollbackState {
    pub objects: i64,
    pub revisions: i64,
    pub receipts: i64,
}

/// Database setup and persistence inspection for the Core transaction test.
pub struct ObjectWriteRollbackFixture;

impl ObjectWriteRollbackFixture {
    pub fn reject_receipt_insert(path: &Path) -> Result<()> {
        Self::with_connection(path, |connection| {
            connection.execute_batch(
                "CREATE TRIGGER reject_object_write_receipt
                 BEFORE INSERT ON entity_write_receipts
                 BEGIN
                     SELECT RAISE(ABORT, 'injected application port failure');
                 END;",
            )
        })
    }

    pub fn allow_receipt_insert(path: &Path) -> Result<()> {
        Self::with_connection(path, |connection| {
            connection.execute_batch("DROP TRIGGER reject_object_write_receipt;")
        })
    }

    pub fn inspect(path: &Path, object_id: &str) -> Result<ObjectWriteRollbackState> {
        Self::with_connection(path, |connection| {
            let objects = connection.query_row(
                "SELECT COUNT(*) FROM objects WHERE id = ?1",
                [object_id],
                |row| row.get(0),
            )?;
            let revisions = connection.query_row(
                "SELECT COUNT(*) FROM object_sync_versions WHERE object_id = ?1",
                [object_id],
                |row| row.get(0),
            )?;
            let receipts = connection.query_row(
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                [object_id],
                |row| row.get(0),
            )?;
            Ok(ObjectWriteRollbackState {
                objects,
                revisions,
                receipts,
            })
        })
    }

    fn with_connection<T>(path: &Path, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let path = path
            .to_str()
            .ok_or(rusqlite::Error::InvalidPath(path.into()))?;
        let connection = crate::open::open_db(path)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))?;
        f(&connection)
    }
}
