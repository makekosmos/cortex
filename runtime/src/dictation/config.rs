// Dictation config. Не-секреты в JSON, API key — в Windows Credential Manager
// через `keyring` (см. spec: enterprise standard, не хранится в plaintext).
//
// Layout: `<data_dir>/dictation-config.json` где data_dir = `MUNDUS_DATA_DIR`
// если установлен, иначе `%APPDATA%\Mundus`.

use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Keyring service name (общий для всего Mundus) + key для Groq API ключа.
/// При добавлении других AI-провайдеров — новый username (service остаётся).
#[cfg(not(test))]
// Persisted keyring service name — holds user API keys; renaming it would
// orphan stored credentials. See docs/brand-legacy-identifiers.md.
const KEYRING_SERVICE: &str = "kosmos-kepler";
#[cfg(not(test))]
const KEYRING_USER_GROQ: &str = "groq-api-key";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// Phase 1: первый press запускает запись, второй — останавливает.
    #[default]
    Toggle,
    /// Phase 1.5 (требует low-level hook): hold-to-record.
    PushToTalk,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InjectMode {
    /// Default: transcript вставляется в активное окно и остаётся в clipboard.
    #[default]
    AutoPaste,
    /// Только записать в буфер обмена, пользователь сам жмёт Ctrl+V.
    ClipboardOnly,
}

/// DNS-резолвер для исходящих AI-запросов. Scope ограничен AI HTTP клиентом
/// (не sync/RAWG/прочее) — см. forbidden.md → Dictation.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkProfile {
    #[default]
    System,
    CloudflareDoh,
    GoogleDoh,
    CustomDoh {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DictationConfig {
    /// Accelerator string в Electron-формате: `Ctrl+Shift+;`. Регистрируется
    /// через `globalShortcut.register` на стороне shell (Toggle mode) или
    /// через low-level Win32 hook (Push-to-talk mode).
    pub hotkey: String,
    pub trigger_mode: TriggerMode,
    /// Whisper language hint: `ru`, `en`, `auto`. `auto` → empty string в API.
    pub language: String,
    pub inject_mode: InjectMode,
    pub network_profile: NetworkProfile,
    /// HTTP/SOCKS proxy URL для AI-провайдеров. Используется когда DoH
    /// недостаточно (SNI-block / TCP block). Поддерживает `http://`,
    /// `https://`, `socks5://`. Пустая строка / None → без proxy.
    /// Scope: только dictation HTTP, не sync/RAWG/прочее.
    pub http_proxy: Option<String>,
    /// Контекстный prompt для Whisper. Пустая строка пропускается. Полезно
    /// для domain-specific терминов / имён / стиля транскрипции.
    /// См. https://platform.openai.com/docs/guides/speech-to-text/prompting
    pub transcription_prompt: String,
    pub provider: String,
    /// Ключ может быть сохранён в keyring, но временно выключен пользователем.
    /// Это UI-level availability switch, не удаление секрета.
    pub provider_enabled: bool,
    pub model: String,
    /// Локальный runtime для on-device STT. MVP: external `whisper.cpp`.
    pub local_engine: String,
    /// User-provided путь к локальной модели Whisper / совместимому весу.
    #[serde(default)]
    pub local_model_path: Option<String>,
    /// User-provided путь к локальному STT executable (`whisper-cli`, `main.exe`).
    #[serde(default)]
    pub local_command_path: Option<String>,
    /// UI/model identifier для локального движка. Храним отдельно от cloud
    /// `model`, потому что у local path и model-id разные жизненные циклы.
    #[serde(default)]
    pub local_model: Option<String>,
    /// `deviceId` микрофона из `navigator.mediaDevices.enumerateDevices()`.
    /// `None` или пустая строка — использовать системный default. Хранится в
    /// JSON чтобы выбор пользователя пережил рестарт shell'а.
    #[serde(default)]
    pub microphone_device_id: Option<String>,
    /// Lower system output volume while dictation is recording.
    #[serde(default)]
    pub duck_audio_during_recording: bool,
    /// Через сколько мс простоя выгружать whisper-server процесс (освободить
    /// VRAM / RAM). `Some(ms)` = выгрузить через ms миллисекунд бездействия;
    /// `None` = никогда не выгружать. Default = 10 минут (600 000 мс).
    /// Используется только для direct whisper-server пути (local engine).
    #[serde(default = "default_local_idle_unload_ms")]
    pub local_idle_unload_ms: Option<u64>,
}

fn default_local_idle_unload_ms() -> Option<u64> {
    Some(600_000)
}

impl Default for DictationConfig {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+;".into(),
            trigger_mode: TriggerMode::Toggle,
            language: "ru".into(),
            inject_mode: InjectMode::AutoPaste,
            network_profile: NetworkProfile::System,
            http_proxy: None,
            transcription_prompt: String::new(),
            provider: "groq".into(),
            provider_enabled: true,
            model: "whisper-large-v3".into(),
            local_engine: "whisper.cpp".into(),
            local_model_path: None,
            local_command_path: None,
            local_model: None,
            microphone_device_id: None,
            duck_audio_during_recording: false,
            local_idle_unload_ms: Some(600_000),
        }
    }
}

/// `%APPDATA%\Mundus` или `MUNDUS_DATA_DIR` (тесты, dev-slot). Точно тот же
/// slot-based изоляция за счёт env
/// переменной, которую устанавливает shell через `resolveInstance`.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MUNDUS_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
    base.join("Mundus")
}

pub fn config_path() -> PathBuf {
    data_dir().join("dictation-config.json")
}

pub fn load() -> DictationConfig {
    load_from(&config_path())
}

pub fn load_from(path: &Path) -> DictationConfig {
    if matches!(std::fs::metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
        // See postmortems.md 2026-07-03: updates can leave primary missing while .bak survives.
        let backup = backup_path(path);
        if let Ok(cfg) = read_config_file(&backup) {
            eprintln!(
                "[dictation] WARN config missing for {}; restored backup",
                path.display()
            );
            return cfg;
        }
        if let Some(cfg) = read_config_file_lenient(&backup) {
            eprintln!(
                "[dictation] WARN config missing for {}; recovered backup fields field-by-field",
                path.display()
            );
            return cfg;
        }
        return DictationConfig::default();
    }
    match read_config_file(path) {
        Ok(cfg) => cfg,
        Err(primary_err) => match read_config_file(&backup_path(path)) {
            Ok(cfg) => {
                eprintln!(
                    "[dictation] WARN config load failed for {} ({primary_err}); restored backup",
                    path.display()
                );
                cfg
            }
            Err(backup_err) => {
                // Last resort перед чистым default'ом: спасаем как можно больше
                // полей по отдельности (особенно `hotkey`). Иначе одно
                // bad/unknown-typed поле в JSON обнуляло бы весь конфиг, и
                // пользовательский hotkey «откатывался» к Ctrl+Shift+;.
                if let Some(cfg) = read_config_file_lenient(path)
                    .or_else(|| read_config_file_lenient(&backup_path(path)))
                {
                    eprintln!(
                        "[dictation] WARN config strict load failed for {} ({primary_err}); recovered fields field-by-field",
                        path.display()
                    );
                    return cfg;
                }
                eprintln!(
                    "[dictation] WARN config load failed for {} ({primary_err}); backup failed ({backup_err}); using defaults",
                    path.display()
                );
                DictationConfig::default()
            }
        },
    }
}

/// Best-effort recovery: разбираем JSON как generic object и применяем каждое
/// поле, которое десериализуется, начиная с дефолтов. Одно битое поле (неверный
/// тип и т.п.) теряет только себя, а не сбрасывает весь конфиг. Возвращает
/// `None` только если текст не читается / не валидный JSON (для торн-райтов
/// есть `.bak`-fallback).
fn read_config_file_lenient(path: &Path) -> Option<DictationConfig> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(deserialize_lenient(&value))
}

fn deserialize_lenient(value: &serde_json::Value) -> DictationConfig {
    let mut cfg = DictationConfig::default();
    let Some(obj) = value.as_object() else {
        return cfg;
    };
    macro_rules! field {
        ($key:literal, $target:expr) => {
            if let Some(v) = obj.get($key) {
                if let Ok(parsed) = serde_json::from_value(v.clone()) {
                    $target = parsed;
                }
            }
        };
    }
    // Ключи — camelCase, как пишет `save_to` (serde rename_all = "camelCase").
    field!("hotkey", cfg.hotkey);
    field!("triggerMode", cfg.trigger_mode);
    field!("language", cfg.language);
    field!("injectMode", cfg.inject_mode);
    field!("networkProfile", cfg.network_profile);
    field!("httpProxy", cfg.http_proxy);
    field!("transcriptionPrompt", cfg.transcription_prompt);
    field!("provider", cfg.provider);
    field!("providerEnabled", cfg.provider_enabled);
    field!("model", cfg.model);
    field!("localEngine", cfg.local_engine);
    field!("localModelPath", cfg.local_model_path);
    field!("localCommandPath", cfg.local_command_path);
    field!("localModel", cfg.local_model);
    field!("microphoneDeviceId", cfg.microphone_device_id);
    field!("duckAudioDuringRecording", cfg.duck_audio_during_recording);
    field!("localIdleUnloadMs", cfg.local_idle_unload_ms);
    cfg
}

pub fn save(cfg: &DictationConfig) -> std::io::Result<()> {
    save_to(&config_path(), cfg)
}

pub fn save_to(path: &Path, cfg: &DictationConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    // См. postmortems.md § 2026-06-23.
    write_atomic(path, text.as_bytes())?;
    if let Err(e) = write_atomic(&backup_path(path), text.as_bytes()) {
        eprintln!("[dictation] WARN config backup save failed: {e}");
    }
    Ok(())
}

fn read_config_file(path: &Path) -> Result<DictationConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.bak",
        path.extension().and_then(|s| s.to_str()).unwrap_or("json")
    ))
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(
        ".{}.tmp.{}.{}",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dictation-config.json"),
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

// ponytail: config writes are rare; use per-path locks if write throughput ever matters.
static ATOMIC_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let _guard = ATOMIC_WRITE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let temp_path = atomic_temp_path(path);
    let result = (|| {
        {
            let mut f = std::fs::File::create(&temp_path)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }

        // `rename` cannot replace an existing file on Windows, so every save
        // after the first used to leave the previous dictation settings intact.
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::Storage::FileSystem::{
                MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
            };

            let from: Vec<u16> = temp_path.as_os_str().encode_wide().chain([0]).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
            // SAFETY: both buffers are NUL-terminated and live through the call.
            unsafe {
                MoveFileExW(
                    PCWSTR(from.as_ptr()),
                    PCWSTR(to.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
                .map_err(std::io::Error::other)
            }
        }
        #[cfg(not(windows))]
        {
            std::fs::rename(&temp_path, path)
        }
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp_path);
    }
    result
}

// ---------------------------------------------------------------------------
// Keyring (API key) — отдельно от JSON. Никогда не пишем секрет в JSON.
// ---------------------------------------------------------------------------

#[cfg(not(test))]
fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_GROQ)
}

fn test_api_key_override() -> Option<String> {
    let is_test_like = matches!(std::env::var("MUNDUS_TEST_MODE").as_deref(), Ok("1"))
        || matches!(std::env::var("MUNDUS_HEADLESS").as_deref(), Ok("1"));
    if !is_test_like {
        return None;
    }
    std::env::var("MUNDUS_TEST_GROQ_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
}

#[cfg(not(test))]
pub fn get_api_key() -> Option<String> {
    if let Some(key) = test_api_key_override() {
        return Some(key);
    }
    let entry = keyring_entry().ok()?;
    entry.get_password().ok()
}

#[cfg(test)]
pub fn get_api_key() -> Option<String> {
    test_api_key_override()
}

pub fn has_api_key() -> bool {
    get_api_key().is_some()
}

#[cfg(not(test))]
pub fn set_api_key(key: &str) -> Result<(), keyring::Error> {
    let entry = keyring_entry()?;
    entry.set_password(key)
}

#[cfg(test)]
pub fn set_api_key(_key: &str) -> Result<(), keyring::Error> {
    Ok(())
}

#[cfg(not(test))]
pub fn clear_api_key() -> Result<(), keyring::Error> {
    let entry = keyring_entry()?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        // Если ключа не было — считаем clear успешным.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
pub fn clear_api_key() -> Result<(), keyring::Error> {
    Ok(())
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default, clippy::panic)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_returns_default_when_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("missing.json");
        let cfg = load_from(&path);
        assert_eq!(cfg.hotkey, "Ctrl+Shift+;");
        assert_eq!(cfg.trigger_mode, TriggerMode::Toggle);
        assert_eq!(cfg.language, "ru");
        assert_eq!(cfg.inject_mode, InjectMode::AutoPaste);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        let cfg = DictationConfig {
            hotkey: "Ctrl+Alt+D".into(),
            trigger_mode: TriggerMode::Toggle,
            language: "en".into(),
            inject_mode: InjectMode::ClipboardOnly,
            network_profile: NetworkProfile::CloudflareDoh,
            http_proxy: Some("http://127.0.0.1:8888".into()),
            transcription_prompt: "Mundus Mundus Groq".into(),
            provider: "groq".into(),
            provider_enabled: true,
            model: "whisper-large-v3".into(),
            local_engine: "whisper.cpp".into(),
            local_model_path: Some("C:/models/ggml-base.bin".into()),
            local_command_path: Some("C:/tools/whisper-cli.exe".into()),
            local_model: Some("ggml-base".into()),
            microphone_device_id: None,
            duck_audio_during_recording: true,
            local_idle_unload_ms: Some(60_000),
        };
        save_to(&path, &cfg).expect("save");
        let loaded = load_from(&path);
        assert_eq!(loaded.hotkey, "Ctrl+Alt+D");
        assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
        assert_eq!(loaded.network_profile, NetworkProfile::CloudflareDoh);
        assert_eq!(loaded.http_proxy.as_deref(), Some("http://127.0.0.1:8888"));
        assert_eq!(loaded.transcription_prompt, "Mundus Mundus Groq");
        assert_eq!(loaded.local_engine, "whisper.cpp");
        assert_eq!(
            loaded.local_model_path.as_deref(),
            Some("C:/models/ggml-base.bin")
        );
        assert_eq!(
            loaded.local_command_path.as_deref(),
            Some("C:/tools/whisper-cli.exe")
        );
        assert_eq!(loaded.local_model.as_deref(), Some("ggml-base"));
        assert!(loaded.duck_audio_during_recording);
        // Новое поле: round-trip.
        assert_eq!(loaded.local_idle_unload_ms, Some(60_000));
    }

    #[test]
    fn local_idle_unload_ms_null_roundtrip() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        let mut cfg = DictationConfig::default();
        cfg.local_idle_unload_ms = None; // "Не выгружать"
        save_to(&path, &cfg).expect("save");
        let loaded = load_from(&path);
        assert_eq!(loaded.local_idle_unload_ms, None);
    }

    #[test]
    fn local_idle_unload_ms_default_on_legacy_config() {
        // Конфиг без localIdleUnloadMs должен загрузиться с дефолтом 5 минут.
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("legacy.json");
        std::fs::write(
            &path,
            r#"{"hotkey":"Ctrl+Shift+;","trigger_mode":"toggle","language":"ru","inject_mode":"auto_paste","network_profile":{"kind":"system"},"provider":"groq","model":"whisper-large-v3"}"#,
        )
        .expect("write");
        let loaded = load_from(&path);
        assert_eq!(loaded.local_idle_unload_ms, Some(600_000));
    }

    #[test]
    fn save_then_load_legacy_no_proxy_field() {
        // Конфиги созданные до Phase 1.5 не имеют http_proxy / transcription_prompt —
        // serde с `#[serde(default)]` должен дать defaults без ошибок.
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("legacy.json");
        std::fs::write(
            &path,
            r#"{"hotkey":"Ctrl+Shift+;","trigger_mode":"toggle","language":"ru","inject_mode":"auto_paste","network_profile":{"kind":"system"},"provider":"groq","model":"whisper-large-v3"}"#,
        )
        .expect("write");
        let loaded = load_from(&path);
        assert_eq!(loaded.hotkey, "Ctrl+Shift+;");
        assert!(loaded.http_proxy.is_none());
        assert!(loaded.transcription_prompt.is_empty());
        assert_eq!(loaded.local_engine, "whisper.cpp");
        assert!(loaded.local_model_path.is_none());
        assert!(loaded.local_command_path.is_none());
        assert!(loaded.local_model.is_none());
    }

    #[test]
    fn load_ignores_malformed_json() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("broken.json");
        std::fs::write(&path, "{not json").expect("write");
        let cfg = load_from(&path);
        assert_eq!(cfg.hotkey, "Ctrl+Shift+;");
    }

    #[test]
    fn load_uses_backup_when_primary_is_malformed() {
        // Regression: 2026-06-23. Partial JSON writes must not reset all settings.
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        let mut cfg = DictationConfig::default();
        cfg.hotkey = "Super+H".into();
        cfg.provider = "local".into();
        cfg.model = "whisper-large-v3-turbo".into();
        cfg.inject_mode = InjectMode::ClipboardOnly;
        save_to(&path, &cfg).expect("save");
        std::fs::write(&path, "{truncated").expect("corrupt primary");

        let loaded = load_from(&path);
        assert_eq!(loaded.hotkey, "Super+H");
        assert_eq!(loaded.provider, "local");
        assert_eq!(loaded.model, "whisper-large-v3-turbo");
        assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
    }

    #[test]
    fn load_uses_backup_when_primary_is_missing() {
        // Regression: 2026-07-03. Updates must not reset hotkey if primary config disappears.
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        let mut cfg = DictationConfig::default();
        cfg.hotkey = "Shift+PageUp".into();
        cfg.provider = "local".into();
        save_to(&path, &cfg).expect("save");
        std::fs::remove_file(&path).expect("remove primary");

        let loaded = load_from(&path);
        assert_eq!(loaded.hotkey, "Shift+PageUp");
        assert_eq!(loaded.provider, "local");
    }

    #[test]
    fn lenient_recovery_preserves_hotkey_when_one_field_is_bad_typed() {
        // Regression: одно поле неверного типа (localIdleUnloadMs как строка)
        // не должно сбрасывать весь конфиг и терять пользовательский hotkey.
        // Backup отсутствует (нет .bak), поэтому strict primary + strict backup
        // оба фейлятся → срабатывает field-by-field recovery.
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        std::fs::write(
            &path,
            r#"{"hotkey":"Shift+PageUp","triggerMode":"toggle","provider":"local","model":"whisper-large-v3-turbo","injectMode":"clipboard_only","localIdleUnloadMs":"not-a-number"}"#,
        )
        .expect("write");

        let loaded = load_from(&path);
        // Спасённые поля.
        assert_eq!(loaded.hotkey, "Shift+PageUp");
        assert_eq!(loaded.provider, "local");
        assert_eq!(loaded.model, "whisper-large-v3-turbo");
        assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
        // Битое поле падает на дефолт, остальное цело.
        assert_eq!(loaded.local_idle_unload_ms, Some(600_000));
    }

    #[test]
    fn save_writes_backup_copy() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        save_to(&path, &DictationConfig::default()).expect("save");
        assert!(backup_path(&path).exists());
    }

    #[test]
    fn atomic_temp_paths_are_unique_per_write() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        assert_ne!(atomic_temp_path(&path), atomic_temp_path(&path));
    }

    #[test]
    fn concurrent_saves_leave_valid_files_and_no_temps() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            for index in 0..8 {
                let path = &path;
                let barrier = &barrier;
                scope.spawn(move || {
                    let cfg = DictationConfig {
                        hotkey: format!("Ctrl+Shift+{index}"),
                        ..Default::default()
                    };
                    barrier.wait();
                    save_to(path, &cfg).expect("concurrent save");
                });
            }
        });

        for saved_path in [&path, &backup_path(&path)] {
            let bytes = std::fs::read(saved_path).expect("saved config");
            serde_json::from_slice::<DictationConfig>(&bytes).expect("valid config JSON");
        }
        assert!(std::fs::read_dir(tmp.path())
            .expect("read tempdir")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")));
    }

    #[test]
    fn failed_atomic_replace_removes_temp_file() {
        let tmp = TempDir::new().expect("tempdir");
        let destination = tmp.path().join("destination");
        std::fs::create_dir(&destination).expect("destination directory");
        assert!(write_atomic(&destination, b"config").is_err());
        assert!(std::fs::read_dir(tmp.path())
            .expect("read tempdir")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")));
    }

    #[test]
    fn save_replaces_existing_config() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        save_to(&path, &DictationConfig::default()).expect("first save");

        let cfg = DictationConfig {
            hotkey: "Ctrl+Shift+K".into(),
            inject_mode: InjectMode::ClipboardOnly,
            ..Default::default()
        };
        save_to(&path, &cfg).expect("second save");

        let loaded = load_from(&path);
        assert_eq!(loaded.hotkey, "Ctrl+Shift+K");
        assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
    }

    #[test]
    fn custom_doh_roundtrip() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cfg.json");
        let cfg = DictationConfig {
            network_profile: NetworkProfile::CustomDoh {
                url: "https://comss.dns.controld.com/dns-query".into(),
            },
            ..Default::default()
        };
        save_to(&path, &cfg).expect("save");
        let loaded = load_from(&path);
        match loaded.network_profile {
            NetworkProfile::CustomDoh { url } => {
                assert!(url.contains("comss"));
            }
            other => panic!("expected CustomDoh, got {other:?}"),
        }
    }
}
