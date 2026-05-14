// Singleton enforcement — копия паттерна из services/usage-tracker/src/singleton.rs,
// адаптированная под Kepler. SQLite WAL `BEGIN IMMEDIATE` лочит на уровне OS;
// если другой инстанс держит соединение, наша попытка падает быстро.
//
// AC2 spec: вторая копия Kepler падает с понятным сообщением и exit code != 0.

use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub struct SingletonGuard {
    // Соединение держится живым на всё время процесса; Drop откатывает транзакцию.
    _connection: Connection,
}

#[derive(Debug, thiserror::Error)]
pub enum SingletonError {
    #[error("Failed to create singleton lock directory: {0}")]
    CreateDir(String),
    #[error("Failed to open singleton lock DB: {0}")]
    OpenDb(String),
    #[error("Failed to configure singleton lock: {0}")]
    Configure(String),
    #[error("Another Kepler instance is already running")]
    AlreadyRunning,
}

impl SingletonGuard {
    pub fn acquire(lock_path: &Path) -> Result<Self, SingletonError> {
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|e| SingletonError::CreateDir(e.to_string()))?;
        }

        let connection =
            Connection::open(lock_path).map_err(|e| SingletonError::OpenDb(e.to_string()))?;

        connection
            .busy_timeout(Duration::from_secs(1))
            .map_err(|e| SingletonError::Configure(e.to_string()))?;

        connection
            .execute_batch("PRAGMA journal_mode = WAL; BEGIN IMMEDIATE;")
            .map_err(|_| SingletonError::AlreadyRunning)?;

        Ok(Self {
            _connection: connection,
        })
    }
}

impl Drop for SingletonGuard {
    fn drop(&mut self) {
        let _ = self._connection.execute_batch("ROLLBACK;");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn second_acquire_fails_fast() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("kepler.lock.db");

        let _first = SingletonGuard::acquire(&lock_path).expect("first lock");
        let second = SingletonGuard::acquire(&lock_path);

        assert!(matches!(second, Err(SingletonError::AlreadyRunning)));
    }

    #[test]
    fn re_acquire_after_drop_works() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("kepler.lock.db");

        let first = SingletonGuard::acquire(&lock_path).expect("first lock");
        drop(first);
        let second = SingletonGuard::acquire(&lock_path);
        assert!(second.is_ok(), "should be able to re-acquire after drop");
    }

    #[test]
    fn creates_parent_directory() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("nested").join("dir").join("kepler.lock.db");
        assert!(!lock_path.parent().unwrap().exists());

        let _guard = SingletonGuard::acquire(&lock_path).expect("acquire creates parent");
        assert!(lock_path.parent().unwrap().exists());
    }
}
