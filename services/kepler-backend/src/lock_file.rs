// Lock-файл для discovery: Electron-апки находят Kepler через kepler.lock.json
// (исторически имя) в %APPDATA%\Kosmos\.
//
// AC3 spec: lock-файл должен быть нечитаем для другого user account на той же машине.
// Achieved через:
//   Windows — `icacls` сброс наследования + grant только current user (SID:F).
//   Unix    — chmod 0600.
//
// Запись атомарная: temp+rename, чтобы апка не прочитала half-written JSON.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;

use crate::protocol_version::ProtocolVersion;

/// Текущая версия формата lock-файла (НЕ протокола). Если поменяется shape JSON —
/// инкрементить, апки старого MAJOR должны отказаться парсить.
pub const LOCK_FILE_FORMAT_VERSION: u32 = 1;

/// Имя файла по умолчанию — `kepler.lock.json` в `%APPDATA%\Kosmos\` (Windows)
/// или `~/.config/Kosmos/` (Linux/macOS). Resolve в lock_file_path().
pub const LOCK_FILE_NAME: &str = "kepler.lock.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeplerLockFile {
    pub format_version: u32,
    pub protocol_version: ProtocolVersion,
    pub pid: u32,
    pub ws_port: u16,
    pub auth_token: String,
    pub started_at: String,
    pub db_path: String,
}

#[derive(Debug, Error)]
pub enum LockFileError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON serialize/deserialize: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Failed to apply OS permissions: {0}")]
    Permissions(String),
}

/// Resolve %APPDATA%\Kosmos\kepler.lock.json (Win) / $XDG_CONFIG_HOME/Kosmos/... (Unix).
///
/// Test override: если выставлен `KOSMOS_DATA_DIR` env, она полностью заменяет
/// base directory (lock-файл, singleton, ark.db — всё под этим dir). Это
/// единственный безопасный способ переопределить путь в Playwright/e2e тестах
/// — иначе тесты случайно укажут на реальный user data dir и потрут данные.
pub fn default_lock_file_path() -> Result<std::path::PathBuf, LockFileError> {
    let base = kosmos_data_dir()?;
    Ok(base.join(LOCK_FILE_NAME))
}

/// Resolve base directory для всех Kosmos backend файлов: lock, singleton,
/// дефолтный ark.db. Уважает `KOSMOS_DATA_DIR` env override (тесты),
/// иначе — `%APPDATA%\Kosmos` (Win) / `$XDG_CONFIG_HOME/Kosmos` (Unix).
pub fn kosmos_data_dir() -> Result<std::path::PathBuf, LockFileError> {
    if let Ok(override_dir) = std::env::var("KOSMOS_DATA_DIR") {
        if !override_dir.is_empty() {
            return Ok(std::path::PathBuf::from(override_dir));
        }
    }
    kosmos_config_dir()
}

#[cfg(windows)]
fn kosmos_config_dir() -> Result<std::path::PathBuf, LockFileError> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| LockFileError::Permissions("%APPDATA% not set".into()))?;
    Ok(std::path::PathBuf::from(appdata).join("Kosmos"))
}

#[cfg(unix)]
fn kosmos_config_dir() -> Result<std::path::PathBuf, LockFileError> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(std::path::PathBuf::from(xdg).join("Kosmos"));
    }
    let home = std::env::var("HOME")
        .map_err(|_| LockFileError::Permissions("$HOME not set".into()))?;
    Ok(std::path::PathBuf::from(home).join(".config").join("Kosmos"))
}

/// Атомарная запись lock-файла. Создаёт parent dir, пишет в temp, fsyncит, переименовывает,
/// применяет strict OS permissions (only current user может прочитать).
pub fn write_atomic(path: &Path, lock: &KeplerLockFile) -> Result<(), LockFileError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_vec_pretty(lock)?;

    // Temp в той же директории — rename atomic только в пределах одной FS.
    let temp_name = format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("kepler.lock.json"),
        std::process::id()
    );
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = parent.join(temp_name);

    // Write + fsync.
    {
        use io::Write as _;
        let mut f = fs::File::create(&temp_path)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }

    // Apply permissions ДО rename — потом файл уже виден через target path.
    apply_owner_only_permissions(&temp_path)?;

    // Atomic rename (overwrites existing на Win и Unix).
    fs::rename(&temp_path, path)?;

    Ok(())
}

#[cfg(unix)]
fn apply_owner_only_permissions(path: &Path) -> Result<(), LockFileError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perms).map_err(|e| LockFileError::Permissions(e.to_string()))
}

#[cfg(windows)]
fn apply_owner_only_permissions(path: &Path) -> Result<(), LockFileError> {
    use std::process::Command;
    // icacls: сначала сбросить inheritance, потом убрать стандартные groups,
    // оставить только current user с full access.
    // /inheritance:r — disable inheritance and remove inherited ACEs
    // /grant:r %USERNAME%:F — replace any existing ACE for current user with Full
    // На современных Windows %USERDOMAIN%\%USERNAME% или просто %USERNAME% работает.
    let username = std::env::var("USERNAME")
        .map_err(|_| LockFileError::Permissions("%USERNAME% not set".into()))?;

    let path_str = path.to_str().ok_or_else(|| {
        LockFileError::Permissions("path is not valid UTF-8".into())
    })?;

    let status = Command::new("icacls")
        .args([
            path_str,
            "/inheritance:r",
            "/grant:r",
            &format!("{}:F", username),
        ])
        .output()
        .map_err(|e| LockFileError::Permissions(format!("icacls spawn failed: {e}")))?;

    if !status.status.success() {
        return Err(LockFileError::Permissions(format!(
            "icacls failed (exit {}): {}",
            status.status,
            String::from_utf8_lossy(&status.stderr)
        )));
    }
    Ok(())
}

pub fn read(path: &Path) -> Result<KeplerLockFile, LockFileError> {
    let bytes = fs::read(path)?;
    let lock: KeplerLockFile = serde_json::from_slice(&bytes)?;
    Ok(lock)
}

/// Прочитать lock-файл и проверить, что записанный PID жив. Если PID мёртв,
/// lock считается stale → возвращаем Ok(None), вызывающий должен удалить файл
/// и стартовать заново.
pub fn read_if_alive(path: &Path) -> Result<Option<KeplerLockFile>, LockFileError> {
    let lock = match read(path) {
        Ok(l) => l,
        Err(LockFileError::Io(e)) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if is_pid_alive(lock.pid) {
        Ok(Some(lock))
    } else {
        Ok(None)
    }
}

#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

#[cfg(windows)]
fn is_pid_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) if !h.is_invalid() => {
                let _ = CloseHandle(h);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_lock(pid: u32, port: u16) -> KeplerLockFile {
        KeplerLockFile {
            format_version: LOCK_FILE_FORMAT_VERSION,
            protocol_version: ProtocolVersion::CURRENT,
            pid,
            ws_port: port,
            auth_token: "deadbeef".repeat(8),
            started_at: "2026-05-13T15:00:00Z".into(),
            db_path: "C:\\Users\\Kazui\\AppData\\Roaming\\Kosmos\\ark.db".into(),
        }
    }

    #[test]
    fn kosmos_data_dir_respects_env_override() {
        // SAFETY: env var, доступ серилизуется через std::env api.
        // Other tests в этом mod не читают KOSMOS_DATA_DIR.
        let dir = tempdir().unwrap();
        let override_path = dir.path().to_path_buf();
        std::env::set_var("KOSMOS_DATA_DIR", &override_path);
        let resolved = kosmos_data_dir().expect("data dir resolves with override");
        std::env::remove_var("KOSMOS_DATA_DIR");
        assert_eq!(resolved, override_path);

        let lock_path = default_lock_file_path_with_env(Some(override_path.clone()));
        assert_eq!(lock_path, override_path.join(LOCK_FILE_NAME));
    }

    // Хелпер только для тестов — детерминированно вычисляет lock-path без env race.
    fn default_lock_file_path_with_env(override_dir: Option<std::path::PathBuf>) -> std::path::PathBuf {
        match override_dir {
            Some(p) => p.join(LOCK_FILE_NAME),
            None => kosmos_config_dir().unwrap().join(LOCK_FILE_NAME),
        }
    }

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("kepler.lock.json");
        let lock = sample_lock(std::process::id(), 12345);

        write_atomic(&path, &lock).unwrap();
        let read_back = read(&path).unwrap();
        assert_eq!(lock, read_back);
    }

    #[test]
    fn read_nonexistent_returns_notfound() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("doesnotexist.json");
        match read(&path) {
            Err(LockFileError::Io(e)) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn read_if_alive_returns_some_for_current_process() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("kepler.lock.json");
        let lock = sample_lock(std::process::id(), 12345);
        write_atomic(&path, &lock).unwrap();

        let result = read_if_alive(&path).unwrap();
        assert_eq!(result, Some(lock));
    }

    #[test]
    fn read_if_alive_returns_none_for_dead_pid() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("kepler.lock.json");
        // PID который точно не существует.
        let lock = sample_lock(0x7FFFFFFF, 12345);
        write_atomic(&path, &lock).unwrap();

        let result = read_if_alive(&path).unwrap();
        assert!(result.is_none(), "dead PID should produce stale → None");
    }

    #[test]
    fn read_if_alive_returns_none_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.lock.json");
        let result = read_if_alive(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("kepler.lock.json");
        assert!(!path.parent().unwrap().exists());

        let lock = sample_lock(std::process::id(), 12345);
        write_atomic(&path, &lock).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn write_overwrites_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("kepler.lock.json");
        write_atomic(&path, &sample_lock(123, 1111)).unwrap();
        write_atomic(&path, &sample_lock(456, 2222)).unwrap();

        let read_back = read(&path).unwrap();
        assert_eq!(read_back.pid, 456);
        assert_eq!(read_back.ws_port, 2222);
    }

    #[cfg(unix)]
    #[test]
    fn unix_permissions_are_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("kepler.lock.json");
        write_atomic(&path, &sample_lock(std::process::id(), 12345)).unwrap();

        let meta = fs::metadata(&path).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600 perms, got {:o}", mode);
    }

    #[cfg(windows)]
    #[test]
    fn windows_acl_inheritance_disabled() {
        // Спецификация AC3: ACL не должен содержать наследованных ACE для
        // BUILTIN\Users или Authenticated Users. Проверяем через icacls /verify.
        let dir = tempdir().unwrap();
        let path = dir.path().join("kepler.lock.json");
        write_atomic(&path, &sample_lock(std::process::id(), 12345)).unwrap();

        let output = std::process::Command::new("icacls")
            .arg(path.to_str().unwrap())
            .output()
            .expect("icacls should be on PATH on Windows");

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("BUILTIN\\Users"),
            "BUILTIN\\Users присутствует в ACL:\n{stdout}"
        );
        assert!(
            !stdout.contains("Authenticated Users"),
            "Authenticated Users присутствуют в ACL:\n{stdout}"
        );
    }
}
