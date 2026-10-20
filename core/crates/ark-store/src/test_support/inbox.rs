use rusqlite::{Connection, Result};

pub fn has_inbox_receipt(connection: &Connection, message_id: &str) -> Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM ark_sync_inbox WHERE message_id = ?1)",
        [message_id],
        |row| row.get(0),
    )
}
