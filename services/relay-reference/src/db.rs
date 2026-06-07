//! SQLite event log for the relay server.
//! Stores all sync entities so new devices can catch up on connect.

use rusqlite::{params, Connection, Result};

pub struct Database {
    conn: Connection,
}

pub struct EventRow {
    pub id: i64,
    #[allow(dead_code)]
    pub space_id: String,
    #[allow(dead_code)]
    pub device_id: String,
    pub entity_json: String,
    #[allow(dead_code)]
    pub hlc: String,
    #[allow(dead_code)]
    pub stored_at: i64,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let db = Database { conn };
        db.init_schema()?;
        Ok(db)
    }

    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                space_id   TEXT NOT NULL,
                device_id  TEXT NOT NULL,
                entity_json TEXT NOT NULL,
                hlc        TEXT NOT NULL,
                stored_at  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_events_space ON events (space_id, stored_at);",
        )?;
        Ok(())
    }

    pub fn store_event(
        &self,
        space_id: &str,
        device_id: &str,
        entity_json: &str,
        hlc: &str,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        self.conn.execute(
            "INSERT INTO events (space_id, device_id, entity_json, hlc, stored_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![space_id, device_id, entity_json, hlc, now],
        )?;
        Ok(())
    }

    /// Return all events for a space since (exclusive) the given HLC.
    /// Pass an empty string to get all events.
    pub fn get_events_since(&self, space_id: &str, since_hlc: &str) -> Result<Vec<EventRow>> {
        let mut stmt = if since_hlc.is_empty() {
            self.conn.prepare(
                "SELECT id, space_id, device_id, entity_json, hlc, stored_at
                 FROM events WHERE space_id = ?1 ORDER BY stored_at ASC",
            )?
        } else {
            self.conn.prepare(
                "SELECT id, space_id, device_id, entity_json, hlc, stored_at
                 FROM events WHERE space_id = ?1 AND hlc > ?2 ORDER BY stored_at ASC",
            )?
        };

        let rows = if since_hlc.is_empty() {
            stmt.query_map(params![space_id], |row| {
                Ok(EventRow {
                    id: row.get(0)?,
                    space_id: row.get(1)?,
                    device_id: row.get(2)?,
                    entity_json: row.get(3)?,
                    hlc: row.get(4)?,
                    stored_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?
        } else {
            stmt.query_map(params![space_id, since_hlc], |row| {
                Ok(EventRow {
                    id: row.get(0)?,
                    space_id: row.get(1)?,
                    device_id: row.get(2)?,
                    entity_json: row.get(3)?,
                    hlc: row.get(4)?,
                    stored_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?
        };

        Ok(rows)
    }
}
