// Lock-файл для discovery: desktop-апки находят Engine через engine.lock.json
// в %APPDATA%\Mundus\.
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
pub const ENGINE_LOCK_FILE_NAME: &str = "engine.lock.json";
pub const ENGINE_LOCK_FILE_FORMAT_VERSION: u32 = 1;

/// Env-флаг (test-only): если выставлен в `1`, hardening permissions
/// (`icacls /inheritance:r ...` на Win / `chmod 0600` на Unix) пропускается.
/// Нужен для e2e — иначе stale lock-файл от прошлого Windows account'а
/// блокирует `freshDataDir` с `EPERM`. Prod НИКОГДА не должен выставлять
/// этот флаг (lock содержит auth token, без ACL он читаем любым процессом
/// текущей машины).
pub const LOCK_PERMISSIONS_DISABLED_ENV: &str = "MUNDUS_LOCK_PERMISSIONS_DISABLED";

fn lock_permissions_disabled() -> bool {
    crate::brand::env("LOCK_PERMISSIONS_DISABLED").as_deref() == Some("1")
}

/// Транзиентные fs-ошибки Windows: AV/индексер кратковременно держит хэндл на
/// свежесозданных файлах, и rename/remove/copy падают с access/sharing ошибками.
/// Ретраим с bounded backoff; последняя ошибка возвращается как есть.
pub(crate) fn retry_io<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    const ATTEMPTS: u32 = 20;
    for attempt in 0..ATTEMPTS {
        match op() {
            Err(error) if is_transient_io(&error) && attempt + 1 < ATTEMPTS => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            result => return result,
        }
    }
    unreachable!("retry_io всегда возвращается из цикла попыток")
}

fn is_transient_io(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::PermissionDenied {
        return true;
    }
    // ERROR_SHARING_VIOLATION (32) / ERROR_DIR_NOT_EMPTY (145) /
    // ERROR_USER_MAPPED_FILE (1224)
    matches!(error.raw_os_error(), Some(32 | 145 | 1224))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineLockFile {
    pub format_version: u32,
    pub api_version: ProtocolVersion,
    pub pid: u32,
    pub http_port: u16,
    pub ws_port: u16,
    pub auth_token: String,
    pub started_at: String,
    pub correlation_id: String,
    /// Product version this Engine was built as part of (KOS-233: Engine no
    /// longer has its own release line). Empty for a build that did not go
    /// through `desktop/scripts/build-backend.mjs`. `#[serde(default)]` so a
    /// lock-file written by an older Engine still parses.
    #[serde(default)]
    pub engine_version: String,
    /// Git commit this Engine was built from. Same fallback rules as
    /// `engine_version`.
    #[serde(default)]
    pub source_commit: String,
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

/// Resolve the Engine data directory (Win / Unix).
///
/// Test override: если выставлен `MUNDUS_DATA_DIR` env (legacy `MUNDUS_DATA_DIR`
/// принимается как fallback), она полностью заменяет base directory (lock-файл,
/// singleton, ark.db — всё под этим dir). Это единственный безопасный способ
/// переопределить путь в тестах — иначе тесты случайно укажут на реальный
/// user data dir и потрут данные.
pub fn default_engine_lock_file_path() -> Result<std::path::PathBuf, LockFileError> {
    Ok(crate::data_dir::mundus_data_dir()?.join(ENGINE_LOCK_FILE_NAME))
}

/// Resolve base directory для всех Mundus backend файлов: lock, singleton,
/// дефолтный ark.db. Уважает `MUNDUS_DATA_DIR` env override (тесты; legacy
/// `MUNDUS_DATA_DIR` fallback), иначе — `%APPDATA%\Mundus` (Win) /
/// `$XDG_CONFIG_HOME/Mundus` (Unix). Учитывает brand-migration: если переезд
/// с `%APPDATA%\Kosmos` не удался, эта сессия работает на legacy dir.
pub fn mundus_data_dir() -> Result<std::path::PathBuf, LockFileError> {
    crate::data_dir::mundus_data_dir()
}

/// Атомарная запись lock-файла. Создаёт parent dir, пишет в temp, fsyncит, переименовывает,
/// применяет strict OS permissions (only current user может прочитать).
pub fn write_engine_atomic(path: &Path, lock: &EngineLockFile) -> Result<(), LockFileError> {
    write_owner_only_json(path, lock)
}

pub(crate) fn write_owner_only_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), LockFileError> {
    if let Some(parent) = path.parent() {
        retry_io(|| fs::create_dir_all(parent))?;
    }

    let json = serde_json::to_vec_pretty(value)?;

    // Temp в той же директории — rename atomic только в пределах одной FS.
    let temp_name = format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("engine.lock.json"),
        std::process::id()
    );
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = parent.join(temp_name);

    // Write + fsync.
    {
        use io::Write as _;
        let mut f = retry_io(|| fs::File::create(&temp_path))?;
        f.write_all(&json)?;
        f.sync_all()?;
    }

    // Apply permissions ДО rename — потом файл уже виден через target path.
    apply_owner_only_permissions(&temp_path)?;

    // Atomic rename (overwrites existing на Win и Unix).
    retry_io(|| fs::rename(&temp_path, path))?;

    Ok(())
}

/// Harden a package-private state directory. Production callers must fail
/// closed when the OS cannot apply the owner-only ACL.
pub(crate) fn ensure_owner_only_directory(path: &Path) -> Result<(), io::Error> {
    retry_io(|| fs::create_dir_all(path))?;
    retry_io(|| apply_owner_only_directory_permissions(path))
}

pub(crate) fn read_owner_only_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, io::Error> {
    let bytes = retry_io(|| fs::read(path))?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

/// Apply owner-only permissions to a broker-created private state file.
pub(crate) fn apply_owner_only_file_permissions(path: &Path) -> Result<(), io::Error> {
    apply_owner_only_permissions(path)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))
}

#[cfg(unix)]
fn apply_owner_only_directory_permissions(path: &Path) -> Result<(), io::Error> {
    if lock_permissions_disabled() {
        return Ok(());
    }
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
fn apply_owner_only_directory_permissions(path: &Path) -> Result<(), io::Error> {
    apply_owner_only_permissions(path)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))
}

#[cfg(unix)]
fn apply_owner_only_permissions(path: &Path) -> Result<(), LockFileError> {
    if lock_permissions_disabled() {
        crate::observability::stderr(format!(
            "[mundus-engine] {LOCK_PERMISSIONS_DISABLED_ENV}=1 — chmod 0600 skipped \
             for {} (test-only path, prod должен не выставлять флаг)",
            path.display()
        ));
        return Ok(());
    }
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perms).map_err(|e| LockFileError::Permissions(e.to_string()))
}

#[cfg(windows)]
fn apply_owner_only_permissions(path: &Path) -> Result<(), LockFileError> {
    if lock_permissions_disabled() {
        crate::observability::stderr(format!(
            "[mundus-engine] {LOCK_PERMISSIONS_DISABLED_ENV}=1 — icacls hardening skipped \
             for {} (test-only path, prod должен не выставлять флаг)",
            path.display()
        ));
        return Ok(());
    }
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // icacls: сначала сбросить inheritance, потом убрать стандартные groups,
    // оставить только current user с full access.
    // /inheritance:r — disable inheritance and remove inherited ACEs
    // /grant:r %USERNAME%:F — replace any existing ACE for current user with Full
    // На современных Windows %USERDOMAIN%\%USERNAME% или просто %USERNAME% работает.
    let username = std::env::var("USERNAME")
        .map_err(|_| LockFileError::Permissions("%USERNAME% not set".into()))?;

    let path_str = path
        .to_str()
        .ok_or_else(|| LockFileError::Permissions("path is not valid UTF-8".into()))?;

    let mut command = Command::new("icacls");
    command.creation_flags(CREATE_NO_WINDOW);
    let status = command
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

pub fn read_engine(path: &Path) -> Result<EngineLockFile, LockFileError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_acl_command_is_created_without_console() {
        let production = include_str!("lock_file.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(production.contains("creation_flags(CREATE_NO_WINDOW)"));
    }
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// Сериализует тесты, которые мутируют process-wide env vars
    /// (`MUNDUS_DATA_DIR`, `MUNDUS_LOCK_PERMISSIONS_DISABLED`). Без этого
    /// параллельные тесты cargo могут race на чтение/запись одной переменной.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn sample_engine_lock(pid: u32, http_port: u16, ws_port: u16) -> EngineLockFile {
        EngineLockFile {
            format_version: ENGINE_LOCK_FILE_FORMAT_VERSION,
            api_version: ProtocolVersion::CURRENT,
            pid,
            http_port,
            ws_port,
            auth_token: "deadbeef".repeat(8),
            started_at: "2026-07-28T15:00:00Z".into(),
            correlation_id: "00000000-0000-4000-8000-000000000001".into(),
            engine_version: "0.9.39".into(),
            source_commit: "a".repeat(40),
        }
    }

    #[test]
    fn engine_version_and_source_commit_default_to_empty_for_old_lock_files() {
        // A lock-file written before KOS-233 lacks these fields; a newer
        // Engine reading it (e.g. during an in-place upgrade race) must not
        // fail to parse.
        let dir = tempdir().unwrap();
        let path = dir.path().join(ENGINE_LOCK_FILE_NAME);
        let legacy = serde_json::json!({
            "format_version": ENGINE_LOCK_FILE_FORMAT_VERSION,
            "api_version": ProtocolVersion::CURRENT,
            "pid": std::process::id(),
            "http_port": 12344,
            "ws_port": 12345,
            "auth_token": "deadbeef".repeat(8),
            "started_at": "2026-07-28T15:00:00Z",
            "correlation_id": "00000000-0000-4000-8000-000000000001",
        });
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let read = read_engine(&path).unwrap();
        assert_eq!(read.engine_version, "");
        assert_eq!(read.source_commit, "");
    }

    #[test]
    fn mundus_data_dir_respects_env_override() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: env var, доступ серилизуется через ENV_MUTEX.
        let dir = tempdir().unwrap();
        let override_path = dir.path().to_path_buf();
        std::env::set_var("MUNDUS_DATA_DIR", &override_path);
        let resolved = mundus_data_dir().expect("data dir resolves with override");
        assert_eq!(resolved, override_path);
        assert_eq!(
            default_engine_lock_file_path().unwrap(),
            override_path.join(ENGINE_LOCK_FILE_NAME)
        );
        std::env::remove_var("MUNDUS_DATA_DIR");
    }

    #[test]
    fn mundus_data_dir_accepts_legacy_env_fallback() {
        // MIGRATION(KOS-267): KOSMOS_DATA_DIR stays a fallback until cleanup.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: env var, доступ серилизуется через ENV_MUTEX.
        let dir = tempdir().unwrap();
        let override_path = dir.path().to_path_buf();
        std::env::remove_var("MUNDUS_DATA_DIR");
        std::env::set_var("KOSMOS_DATA_DIR", &override_path); // MIGRATION(KOS-267)
        assert_eq!(
            crate::brand::env("DATA_DIR"),
            Some(override_path.to_string_lossy().into_owned())
        );
        // MUNDUS_ wins over the legacy name when both are set.
        let primary = tempdir().unwrap();
        std::env::set_var("MUNDUS_DATA_DIR", primary.path());
        assert_eq!(
            crate::brand::env("DATA_DIR"),
            Some(primary.path().to_string_lossy().into_owned())
        );
        std::env::remove_var("KOSMOS_DATA_DIR");
        std::env::remove_var("MUNDUS_DATA_DIR");
    }

    #[test]
    fn engine_lock_write_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(ENGINE_LOCK_FILE_NAME);
        let lock = sample_engine_lock(std::process::id(), 12344, 12345);

        write_engine_atomic(&path, &lock).unwrap();
        assert_eq!(read_engine(&path).unwrap(), lock);
    }

    #[test]
    fn production_runtime_does_not_publish_legacy_lock() {
        let source = include_str!("main.rs");
        assert!(!source.contains("write_atomic(&lock_path"));
        assert!(!source.contains("legacy lock-file written"));
    }

    #[cfg(unix)]
    #[test]
    fn unix_permissions_are_0600() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Гарантируем, что флаг отключения hardening не выставлен из другого теста.
        std::env::remove_var(LOCK_PERMISSIONS_DISABLED_ENV);
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join(ENGINE_LOCK_FILE_NAME);
        write_engine_atomic(&path, &sample_engine_lock(std::process::id(), 12345, 12346)).unwrap();

        let meta = fs::metadata(&path).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600 perms, got {:o}", mode);
    }

    #[test]
    fn permissions_disabled_env_skips_hardening() {
        // AC4: при MUNDUS_LOCK_PERMISSIONS_DISABLED=1 запись lock-файла
        // должна пройти без применения hardening (ACL на Win / chmod 0600 на Unix).
        // Файл должен существовать и быть читаемым стандартным путём.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: process-wide env, сериализовано ENV_MUTEX.
        std::env::set_var(LOCK_PERMISSIONS_DISABLED_ENV, "1");

        let dir = tempdir().unwrap();
        let path = dir.path().join(ENGINE_LOCK_FILE_NAME);
        let lock = sample_engine_lock(std::process::id(), 12345, 12346);
        let write_result = write_engine_atomic(&path, &lock);

        // Доп. проверка на Unix: режим НЕ 0600 (а дефолтный umask), потому
        // что мы пропустили chmod.
        #[cfg(unix)]
        let mode_after = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&path)
                .ok()
                .map(|m| m.permissions().mode() & 0o777)
        };

        std::env::remove_var(LOCK_PERMISSIONS_DISABLED_ENV);

        write_result.expect("write_atomic with permissions disabled must succeed");
        assert!(path.exists(), "lock-файл должен быть создан");
        let read_back = read_engine(&path).expect("должен быть читаемый стандартным путём");
        assert_eq!(read_back, lock);

        #[cfg(unix)]
        {
            // 0600 невозможно для дефолтного umask (обычно 0644 или 0664).
            // Если бы hardening не пропустился — было бы ровно 0600.
            let mode = mode_after.expect("mode read");
            assert_ne!(
                mode, 0o600,
                "expected non-0600 mode (hardening skipped), got {:o}",
                mode
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_acl_inheritance_disabled() {
        // Спецификация AC3: ACL не должен содержать наследованных ACE для
        // BUILTIN\Users или Authenticated Users. Проверяем через icacls /verify.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Гарантируем, что флаг отключения hardening не выставлен из другого теста.
        std::env::remove_var(LOCK_PERMISSIONS_DISABLED_ENV);
        let dir = tempdir().unwrap();
        let path = dir.path().join(ENGINE_LOCK_FILE_NAME);
        write_engine_atomic(&path, &sample_engine_lock(std::process::id(), 12345, 12346)).unwrap();

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
