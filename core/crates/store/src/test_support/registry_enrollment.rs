use rusqlite::{Connection, Result};

pub fn reject_device_registry_projection(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TRIGGER reject_device_registry_projection
         BEFORE INSERT ON device_registry_projection
         BEGIN SELECT RAISE(ABORT, 'injected projection failure'); END;",
    )
}

pub fn reject_remote_device_rename(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TRIGGER reject_registry_rename
         BEFORE UPDATE OF display_name ON device_registry_projection
         WHEN NEW.device_id = 'remote-device'
         BEGIN SELECT RAISE(ABORT, 'injected projection failure'); END;",
    )
}
