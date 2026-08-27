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
use std::sync::Mutex;

static CONFIG_WRITE_LOCK: Mutex<()> = Mutex::new(());

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
    /// Invalid launcher state is retained but blocked until a fresh scan or
    /// manual registration replaces it.
    pub quarantined_local_games: std::collections::HashMap<String, String>,
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
    save_with_quarantine(cfg, &[])
}

pub fn save_with_quarantine(
    cfg: &ArrancadorConfig,
    cleared_game_ids: &[String],
) -> std::io::Result<()> {
    let _guard = CONFIG_WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    save_with_quarantine_to(&config_path(), cfg, cleared_game_ids)
}

pub fn record_quarantine(game_id: &str, code: &str) -> std::io::Result<()> {
    let _guard = CONFIG_WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cfg = load();
    cfg.quarantined_local_games
        .insert(game_id.to_string(), code.to_string());
    save_to(&config_path(), &cfg)
}

fn save_with_quarantine_to(
    path: &Path,
    cfg: &ArrancadorConfig,
    cleared_game_ids: &[String],
) -> std::io::Result<()> {
    let latest = load_from(path);
    let mut merged = cfg.clone();
    for (game_id, code) in latest.quarantined_local_games {
        merged
            .quarantined_local_games
            .entry(game_id)
            .or_insert(code);
    }
    for game_id in cleared_game_ids {
        merged.quarantined_local_games.remove(game_id);
    }
    save_to(path, &merged)
}

pub fn save_to(path: &Path, cfg: &ArrancadorConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    let temp_path = path.with_file_name(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config"),
        uuid::Uuid::new_v4()
    ));
    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&temp_path)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from = temp_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let to = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
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
            quarantined_local_games: std::collections::HashMap::from([(
                "game-1".into(),
                "invalid-steam-app-id".into(),
            )]),
        };
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.rawg_api_key.as_deref(), Some("secret-key"));
        assert_eq!(loaded.custom_scan_paths.len(), 1);
        assert_eq!(loaded.keep_backups, Some(5));
        assert_eq!(
            loaded
                .quarantined_local_games
                .get("game-1")
                .map(String::as_str),
            Some("invalid-steam-app-id")
        );
    }

    #[test]
    fn load_ignores_malformed_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("broken.json");
        std::fs::write(&path, "{not json").unwrap();
        let cfg = load_from(&path);
        assert!(cfg.rawg_api_key.is_none());
    }

    #[test]
    fn save_preserves_quarantine_from_a_newer_snapshot() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cfg.json");
        let mut current = ArrancadorConfig::default();
        current
            .quarantined_local_games
            .insert("game-1".into(), "invalid-state".into());
        save_to(&path, &current).unwrap();

        let stale = ArrancadorConfig {
            rawg_api_key: Some("updated".into()),
            ..Default::default()
        };
        save_with_quarantine_to(&path, &stale, &[]).unwrap();

        let loaded = load_from(&path);
        assert_eq!(loaded.rawg_api_key.as_deref(), Some("updated"));
        assert_eq!(
            loaded
                .quarantined_local_games
                .get("game-1")
                .map(String::as_str),
            Some("invalid-state")
        );
    }
}
