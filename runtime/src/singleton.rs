// Singleton enforcement — копия паттерна из services/usage-tracker/src/singleton.rs.
// SQLite WAL `BEGIN IMMEDIATE` лочит на уровне OS;
// если другой инстанс держит соединение, наша попытка падает быстро.
//
// AC2 spec: вторая копия Engine падает с понятным сообщением и exit code != 0.

use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::lock_file::{self, LockFileError};

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
    #[error("Another Engine instance is already running")]
    AlreadyRunning,
    #[error("Failed to clear stale lock file: {0}")]
    ClearStaleLock(String),
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

/// Acquire singleton + удалить stale `engine.lock.json` если он был.
///
/// Почему так: `SingletonGuard` (SQLite WAL exclusive lock на
/// `kepler-singleton.lock.db`) — настоящий OS-level gate, kernel
/// освобождает handle при любой смерти процесса (panic, kill -9, BSOD).
/// JSON-файл — только discovery-метаданные для shell (ws_port, auth_token).
///
/// Раньше startup гейтился на «PID из JSON жив через `OpenProcess`», что
/// ломалось на pid reuse: Windows отдавала освободившийся PID другому
/// процессу (electron, chrome, ...), гейт видел «PID жив» и бесконечно
/// отказывал в старте. См. postmortems.md § 2026-05-23 — Kepler:
/// singleton conflict из-за pid reuse.
///
/// Возвращает `(guard, Some(stale_pid))` если на диске лежал JSON от
/// предыдущего инстанса — для диагностического лога. Сам файл к моменту
/// возврата уже удалён, чтобы shell не успел прочесть stale ws_port в
/// окне между acquire и записью нового JSON.
pub fn acquire_clearing_stale_lock(
    lock_path: &Path,
    singleton_path: &Path,
) -> Result<(SingletonGuard, Option<u32>), SingletonError> {
    let guard = SingletonGuard::acquire(singleton_path)?;

    let stale_pid = match lock_file::read_engine(lock_path) {
        Ok(lock) => Some(lock.pid),
        Err(LockFileError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            // JSON есть, но не парсится (corrupt). Удалим — у нас singleton lock,
            // мы вправе перетереть. Логируем для observability.
            tracing::warn!(error = %e, "stale engine.lock.json corrupt, removing");
            None
        }
    };

    // Удаляем JSON безусловно (если он есть) — невалидный, валидный, наш или
    // не наш. После acquire singleton'а мы единственный backend, никто другой
    // его читать как «свой» не должен.
    if let Err(e) = fs::remove_file(lock_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(SingletonError::ClearStaleLock(e.to_string()));
        }
    }

    Ok((guard, stale_pid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock_file::{write_engine_atomic, EngineLockFile, ENGINE_LOCK_FILE_FORMAT_VERSION};
    use crate::protocol_version::ProtocolVersion;
    use tempfile::tempdir;

    fn sample_lock(pid: u32, port: u16) -> EngineLockFile {
        EngineLockFile {
            format_version: ENGINE_LOCK_FILE_FORMAT_VERSION,
            api_version: ProtocolVersion::CURRENT,
            pid,
            http_port: port,
            ws_port: port,
            auth_token: "deadbeef".repeat(8),
            started_at: "2026-05-23T17:56:07Z".into(),
            correlation_id: "00000000-0000-4000-8000-000000000001".into(),
        }
    }

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

    // Regression: 2026-05-23. Pid reuse: engine.lock.json остался от упавшего
    // backend'а, его PID Windows переиспользовала для unrelated живого процесса
    // (electron, chrome). Старый код гейтил startup на `is_pid_alive(stale.pid)`
    // → backend никогда не стартовал. После фикса: trust SingletonGuard, JSON
    // считается чистыми метаданными для shell, не gate'ом.
    #[test]
    fn stale_lock_with_live_unrelated_pid_does_not_block_acquire() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("engine.lock.json");
        let singleton_path = dir.path().join("engine-singleton.lock.db");

        // PID текущего теста — это cargo test binary, гарантированно живой и
        // гарантированно НЕ kepler-backend. Симулирует pid reuse.
        let stale = sample_lock(std::process::id(), 60803);
        write_engine_atomic(&lock_path, &stale).expect("write stale lock");

        let (_guard, reported_pid) = acquire_clearing_stale_lock(&lock_path, &singleton_path)
            .expect("must acquire despite stale json with live unrelated pid");

        assert_eq!(reported_pid, Some(std::process::id()));
        assert!(
            !lock_path.exists(),
            "stale json должен быть удалён после acquire, чтобы shell не \
             прочёл устаревший ws_port в окне до записи нового JSON"
        );
    }

    #[test]
    fn singleton_uses_only_engine_lock_contract() {
        let source = include_str!("singleton.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("singleton production source");
        assert!(source.contains("read_engine"));
        assert!(!source.contains("lock_file::read("));
        assert!(!source.contains("kepler.lock.json"));
    }

    #[test]
    fn acquire_clearing_stale_lock_handles_missing_json() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("engine.lock.json");
        let singleton_path = dir.path().join("engine-singleton.lock.db");

        let (_guard, reported_pid) = acquire_clearing_stale_lock(&lock_path, &singleton_path)
            .expect("first-time startup без существующего JSON должен пройти");

        assert_eq!(reported_pid, None);
        assert!(!lock_path.exists());
    }

    #[test]
    fn acquire_clearing_stale_lock_removes_corrupt_json() {
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("engine.lock.json");
        let singleton_path = dir.path().join("engine-singleton.lock.db");

        // Crash во время write_atomic мог оставить мусор — наш acquire всё
        // равно должен пройти и снести битый файл.
        std::fs::write(&lock_path, b"{ not valid json").expect("seed corrupt");

        let (_guard, reported_pid) = acquire_clearing_stale_lock(&lock_path, &singleton_path)
            .expect("corrupt json не должен блокировать singleton");

        assert_eq!(reported_pid, None);
        assert!(!lock_path.exists(), "corrupt json должен быть удалён");
    }

    #[test]
    fn second_acquire_clearing_stale_lock_fails_with_already_running() {
        // GAP: in-process only. Два `BEGIN IMMEDIATE` внутри одного процесса
        // конфликтуют через SQLite connection-state, НЕ через OS file lock
        // (fcntl/LockFileEx). Реальная kernel-level гарантия — что handle
        // освобождается на TerminateProcess/SIGKILL без graceful Drop — этим
        // тестом не покрыта.
        //
        // Cross-process валидация запланирована отдельной задачей:
        // .agent/tasks/2026-05-23-singleton-cross-process-test/spec.md.
        let dir = tempdir().expect("temp dir");
        let lock_path = dir.path().join("engine.lock.json");
        let singleton_path = dir.path().join("engine-singleton.lock.db");

        let _first = acquire_clearing_stale_lock(&lock_path, &singleton_path)
            .expect("первый acquire должен пройти");

        let second = acquire_clearing_stale_lock(&lock_path, &singleton_path);
        assert!(matches!(second, Err(SingletonError::AlreadyRunning)));

        // Сообщение об ошибке — часть контракта. Должно быть human-readable.
        let msg = SingletonError::AlreadyRunning.to_string();
        assert!(
            msg.contains("Engine"),
            "ошибка должна упоминать Engine для diagnosability, got: {msg}"
        );
    }
}
