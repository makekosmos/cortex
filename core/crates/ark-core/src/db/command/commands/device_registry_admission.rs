use super::super::DbError;
use rusqlite::{Connection, OptionalExtension, Transaction};

pub(super) fn require_registry_peer_known(
    transaction: &Transaction<'_>,
    device_id: &str,
) -> Result<(), DbError> {
    let known = transaction
        .query_row(
            "SELECT 1 FROM device_registry_projection WHERE device_id = ?1",
            [device_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(storage_error)?
        .is_some();
    if known {
        Ok(())
    } else {
        Err(DbError::Permission("sync_peer_not_registered".into()))
    }
}

pub(super) fn registry_device_is_admitted(
    connection: &Connection,
    device_id: &str,
) -> Result<Option<bool>, DbError> {
    connection
        .query_row(
            "SELECT disabled_event_id IS NULL FROM device_registry_projection
             WHERE device_id = ?1",
            [device_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage_error)
}

fn storage_error(error: rusqlite::Error) -> DbError {
    DbError::Storage(error.to_string())
}
