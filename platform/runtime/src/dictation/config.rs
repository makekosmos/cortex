// Dictation config. Не-секреты в JSON, API key — в Windows Credential Manager
// через `keyring` (см. spec: enterprise standard, не хранится в plaintext).
//
// Layout: `<data_dir>/dictation-config.json` где data_dir = `KOSMOS_DATA_DIR`
// если установлен, иначе `%APPDATA%\Kosmos`. Шаблон из arrancador/config.rs.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Keyring service name (общий для всего Kepler) + key для Groq API ключа.
/// При добавлении других AI-провайдеров — новый username (service остаётся).
#[cfg(not(test))]
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
    /// Default: clipboard сохраняется → текст → Ctrl+V → restore clipboard.
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
    /// `deviceId` микрофона из `navigator.mediaDevices.enumerateDevices()`.
    /// `None` или пустая строка — использовать системный default. Хранится в
    /// JSON чтобы выбор пользователя пережил рестарт shell'а.
    #[serde(default)]
    pub microphone_device_id: Option<String>,
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
            microphone_device_id: None,
        }
    }
}

/// `%APPDATA%\Kosmos` или `KOSMOS_DATA_DIR` (тесты, dev-slot). Точно тот же
/// resolver что в arrancador/config.rs — slot-based изоляция за счёт env
/// переменной, которую устанавливает shell через `resolveInstance`.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("KOSMOS_DATA_DIR") {
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
    base.join("Kosmos")
}

pub fn config_path() -> PathBuf {
    data_dir().join("dictation-config.json")
}

pub fn load() -> DictationConfig {
    load_from(&config_path())
}

pub fn load_from(path: &Path) -> DictationConfig {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => DictationConfig::default(),
    }
}

pub fn save(cfg: &DictationConfig) -> std::io::Result<()> {
    save_to(&config_path(), cfg)
}

pub fn save_to(path: &Path, cfg: &DictationConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

// ---------------------------------------------------------------------------
// Keyring (API key) — отдельно от JSON. Никогда не пишем секрет в JSON.
// ---------------------------------------------------------------------------

#[cfg(not(test))]
fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_GROQ)
}

#[cfg(not(test))]
pub fn get_api_key() -> Option<String> {
    let entry = keyring_entry().ok()?;
    entry.get_password().ok()
}

#[cfg(test)]
pub fn get_api_key() -> Option<String> {
    std::env::var("KOSMOS_TEST_GROQ_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
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
            transcription_prompt: "Kepler Kosmos Groq".into(),
            provider: "groq".into(),
            provider_enabled: true,
            model: "whisper-large-v3".into(),
            microphone_device_id: None,
        };
        save_to(&path, &cfg).expect("save");
        let loaded = load_from(&path);
        assert_eq!(loaded.hotkey, "Ctrl+Alt+D");
        assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
        assert_eq!(loaded.network_profile, NetworkProfile::CloudflareDoh);
        assert_eq!(loaded.http_proxy.as_deref(), Some("http://127.0.0.1:8888"));
        assert_eq!(loaded.transcription_prompt, "Kepler Kosmos Groq");
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
