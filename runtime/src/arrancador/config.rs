// Arrancador shared config — `%APPDATA%\Kosmos\arrancador-config.json`.
//
// SHARED между subagent'ами A/B/C/D — каждый из них владеет своими полями:
//   * `rawg_api_key`           — RAWG client (subagent B)
//   * `custom_scan_paths`      — Scanner (subagent A)
//   * `sqoba_dest_dir`         — SQOBA (subagent C)
//   * `steam_library_override` — Scanner (subagent A) — для test override
//   * `keep_backups`           — SQOBA (subagent C)
//
// Plaintext storage — это user API key + paths, не credentials grade. Если
// потребуется encrypt — `keyring-rs` отдельной фазой (см. spec.md OOS).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ArrancadorConfig {
    pub rawg_api_key: Option<String>,
    pub custom_scan_paths: Vec<PathBuf>,
    pub sqoba_dest_dir: Option<PathBuf>,
    pub steam_library_override: Option<PathBuf>,
    /// Сколько последних бэкапов сохранять на игру (default 10).
    pub keep_backups: Option<u32>,
    /// Device-local launcher metadata. This is deliberately not part of the
    /// canonical Game object or any public Arrancador DTO.
    pub local_games: std::collections::HashMap<String, LocalGameState>,
}

pub type LocalGameState = ark_core::canonical_types::game::GameLocalState;

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
    load_from(&config_path())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_returns_default_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does-not-exist.json");
        let cfg = load_from(&path);
        assert!(cfg.rawg_api_key.is_none());
        assert!(cfg.custom_scan_paths.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cfg.json");
        let cfg = ArrancadorConfig {
            rawg_api_key: Some("secret-key".into()),
            custom_scan_paths: vec![PathBuf::from("C:\\Games")],
            sqoba_dest_dir: Some(PathBuf::from("D:\\backups")),
            steam_library_override: None,
            keep_backups: Some(5),
            local_games: std::collections::HashMap::new(),
        };
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.rawg_api_key.as_deref(), Some("secret-key"));
        assert_eq!(loaded.custom_scan_paths.len(), 1);
        assert_eq!(loaded.keep_backups, Some(5));
    }

    #[test]
    fn load_ignores_malformed_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("broken.json");
        std::fs::write(&path, "{not json").unwrap();
        let cfg = load_from(&path);
        assert!(cfg.rawg_api_key.is_none());
    }
}
