// Arrancador shared config — `%APPDATA%\Kosmos\arrancador-config.json`.
//
// SHARED между subagent'ами A/B/C/D — каждый из них владеет своими полями:
//   * `custom_scan_paths`      — Scanner (subagent A)
//   * `sqoba_dest_dir`         — SQOBA (subagent C)
//   * `steam_library_override` — Scanner (subagent A) — для test override
//   * `keep_backups`           — SQOBA (subagent C)
//
// Пути и настройки хранятся в JSON. Секреты — только в Windows Credential Manager.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ArrancadorConfig {
    pub custom_scan_paths: Vec<PathBuf>,
    pub sqoba_dest_dir: Option<PathBuf>,
    pub steam_library_override: Option<PathBuf>,
    /// Сколько последних бэкапов сохранять на игру (default 10).
    pub keep_backups: Option<u32>,
}

const KEYRING_SERVICE: &str = "kosmos-kepler";
const KEYRING_USER_RAWG: &str = "rawg-api-key";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct LegacyArrancadorConfig {
    rawg_api_key: Option<String>,
}

/// Корневая директория конфигов Kosmos. Respect'ит `KOSMOS_DATA_DIR` env (тесты)
/// и dev-build (`Kosmos-dev`), иначе `%APPDATA%\Kosmos`.
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
    data_dir().join("arrancador-config.json")
}

pub fn load() -> ArrancadorConfig {
    let path = config_path();
    let cfg = load_from(&path);
    migrate_legacy_rawg_key(&path, &cfg);
    cfg
}

pub fn load_from(path: &Path) -> ArrancadorConfig {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => ArrancadorConfig::default(),
    }
}

pub fn save(cfg: &ArrancadorConfig) -> std::io::Result<()> {
    save_to(&config_path(), cfg)
}

pub fn save_to(path: &Path, cfg: &ArrancadorConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(not(test))]
fn rawg_keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_RAWG)
}

#[cfg(not(test))]
pub fn rawg_api_key() -> Option<String> {
    rawg_keyring_entry().ok()?.get_password().ok()
}

#[cfg(test)]
pub fn rawg_api_key() -> Option<String> {
    None
}

#[cfg(not(test))]
pub fn set_rawg_api_key(key: &str) -> Result<(), keyring::Error> {
    rawg_keyring_entry()?.set_password(key)
}

#[cfg(test)]
pub fn set_rawg_api_key(_key: &str) -> Result<(), keyring::Error> {
    Ok(())
}

#[cfg(not(test))]
pub fn clear_rawg_api_key() -> Result<(), keyring::Error> {
    match rawg_keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
pub fn clear_rawg_api_key() -> Result<(), keyring::Error> {
    Ok(())
}

fn migrate_legacy_rawg_key(path: &Path, cfg: &ArrancadorConfig) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(legacy) = serde_json::from_str::<LegacyArrancadorConfig>(&text) else {
        return;
    };
    let Some(key) = legacy.rawg_api_key.filter(|key| !key.trim().is_empty()) else {
        return;
    };
    if set_rawg_api_key(&key).is_ok() {
        let _ = save_to(path, cfg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_returns_default_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does-not-exist.json");
        let cfg = load_from(&path);
        assert!(cfg.custom_scan_paths.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cfg.json");
        let cfg = ArrancadorConfig {
            custom_scan_paths: vec![PathBuf::from("C:\\Games")],
            sqoba_dest_dir: Some(PathBuf::from("D:\\backups")),
            steam_library_override: None,
            keep_backups: Some(5),
        };
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.custom_scan_paths.len(), 1);
        assert_eq!(loaded.keep_backups, Some(5));
    }

    #[test]
    fn load_ignores_malformed_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("broken.json");
        std::fs::write(&path, "{not json").unwrap();
        let cfg = load_from(&path);
        assert!(cfg.custom_scan_paths.is_empty());
    }

    #[test]
    fn save_drops_legacy_rawg_key() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cfg.json");
        std::fs::write(&path, r#"{"rawgApiKey":"secret-key"}"#).unwrap();
        save_to(&path, &load_from(&path)).unwrap();
        assert!(!std::fs::read_to_string(path).unwrap().contains("secret-key"));
    }
}
