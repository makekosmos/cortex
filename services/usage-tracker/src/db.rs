use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct UsageDb {
    path: PathBuf,
}

impl UsageDb {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = Connection::open(&self.path)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        f(&conn)
    }

    pub fn with_transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        self.with_conn(|conn| {
            conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
            let result = f(conn);

            match result {
                Ok(value) => {
                    conn.execute_batch("COMMIT;")?;
                    Ok(value)
                }
                Err(error) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    Err(error)
                }
            }
        })
    }

    pub fn initialize_schema(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS usage_devices (
                    id TEXT PRIMARY KEY,
                    device_name TEXT NOT NULL,
                    platform TEXT NOT NULL,
                    first_seen_at TEXT NOT NULL,
                    last_seen_at TEXT NOT NULL,
                    metadata_json TEXT
                );

                CREATE TABLE IF NOT EXISTS usage_apps (
                    id TEXT PRIMARY KEY,
                    platform TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    process_name TEXT NOT NULL,
                    normalized_exe_path TEXT,
                    executable_path TEXT,
                    first_seen_at TEXT NOT NULL,
                    last_seen_at TEXT NOT NULL,
                    metadata_json TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_usage_apps_platform
                    ON usage_apps(platform);
                CREATE INDEX IF NOT EXISTS idx_usage_apps_normalized_exe_path
                    ON usage_apps(normalized_exe_path);

                CREATE TABLE IF NOT EXISTS usage_sessions (
                    id TEXT PRIMARY KEY,
                    device_id TEXT NOT NULL,
                    app_id TEXT,
                    usage_kind TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    last_seen_at TEXT NOT NULL,
                    ended_at TEXT,
                    foreground_ms INTEGER NOT NULL DEFAULT 0,
                    idle_ms INTEGER NOT NULL DEFAULT 0,
                    sample_count INTEGER NOT NULL DEFAULT 0,
                    window_title TEXT,
                    process_name TEXT,
                    executable_path TEXT,
                    normalized_exe_path TEXT,
                    pid_start INTEGER,
                    pid_end INTEGER,
                    metadata_json TEXT,
                    FOREIGN KEY(device_id) REFERENCES usage_devices(id),
                    FOREIGN KEY(app_id) REFERENCES usage_apps(id)
                );

                CREATE INDEX IF NOT EXISTS idx_usage_sessions_device_id
                    ON usage_sessions(device_id);
                CREATE INDEX IF NOT EXISTS idx_usage_sessions_app_id
                    ON usage_sessions(app_id);
                CREATE INDEX IF NOT EXISTS idx_usage_sessions_usage_kind
                    ON usage_sessions(usage_kind);

                CREATE TABLE IF NOT EXISTS usage_events (
                    id TEXT PRIMARY KEY,
                    device_id TEXT NOT NULL,
                    app_id TEXT,
                    session_id TEXT,
                    occurred_at TEXT NOT NULL,
                    usage_kind TEXT NOT NULL,
                    event_kind TEXT NOT NULL,
                    sample_ms INTEGER NOT NULL,
                    foreground_ms INTEGER NOT NULL DEFAULT 0,
                    idle_ms INTEGER NOT NULL DEFAULT 0,
                    window_title TEXT,
                    process_name TEXT,
                    executable_path TEXT,
                    normalized_exe_path TEXT,
                    pid INTEGER,
                    is_foreground INTEGER NOT NULL,
                    is_idle INTEGER NOT NULL,
                    metadata_json TEXT,
                    FOREIGN KEY(device_id) REFERENCES usage_devices(id),
                    FOREIGN KEY(app_id) REFERENCES usage_apps(id),
                    FOREIGN KEY(session_id) REFERENCES usage_sessions(id)
                );

                CREATE INDEX IF NOT EXISTS idx_usage_events_device_id
                    ON usage_events(device_id);
                CREATE INDEX IF NOT EXISTS idx_usage_events_session_id
                    ON usage_events(session_id);
                CREATE INDEX IF NOT EXISTS idx_usage_events_occurred_at
                    ON usage_events(occurred_at);
                "#,
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UsageDb;
    use tempfile::NamedTempFile;

    #[test]
    fn initialize_schema_creates_usage_tables() {
        let file = NamedTempFile::new().expect("temp db");
        let db = UsageDb::new(file.path());

        db.initialize_schema().expect("initialize schema");

        db.with_conn(|conn| {
            for table in [
                "usage_devices",
                "usage_apps",
                "usage_sessions",
                "usage_events",
            ] {
                let exists: i64 = conn.query_row(
                    "SELECT EXISTS(
                        SELECT 1
                        FROM sqlite_master
                        WHERE type = 'table' AND name = ?1
                    )",
                    [table],
                    |row| row.get(0),
                )?;
                assert_eq!(exists, 1, "table {table} should exist");
            }

            Ok(())
        })
        .expect("inspect schema");
    }
}
