use rusqlite::{Connection, Result};

pub fn reject_paired_device_projection(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TRIGGER reject_device_registry_projection
         BEFORE INSERT ON device_registry_projection
         WHEN NEW.device_id = 'paired-device'
         BEGIN SELECT RAISE(ABORT, 'injected projection failure'); END;",
    )
}
