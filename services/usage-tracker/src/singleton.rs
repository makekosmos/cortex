use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub struct SingletonGuard {
    connection: Connection,
}

impl SingletonGuard {
    pub fn acquire(lock_path: &Path) -> Result<Self, String> {
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create lock directory: {error}"))?;
        }

        let connection = Connection::open(lock_path)
            .map_err(|error| format!("Failed to open singleton lock: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(1))
            .map_err(|error| format!("Failed to configure singleton lock timeout: {error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode = WAL; BEGIN IMMEDIATE;")
            .map_err(|error| format!("Another usage-tracker instance is already running: {error}"))?;

        Ok(Self { connection })
    }
}

impl Drop for SingletonGuard {
    fn drop(&mut self) {
        let _ = self.connection.execute_batch("ROLLBACK;");
    }
}

#[cfg(test)]
mod tests {
    use super::SingletonGuard;
    use tempfile::tempdir;

    #[test]
    fn second_acquire_fails_fast() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("usage-tracker.lock.db");

        let guard = SingletonGuard::acquire(&lock_path).expect("first lock");
        let second = SingletonGuard::acquire(&lock_path);

        assert!(second.is_err());
        drop(guard);
    }
}
