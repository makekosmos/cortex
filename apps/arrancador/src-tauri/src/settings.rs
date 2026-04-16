use crate::database::{with_db, with_db_mut};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub ludusavi_path: String,
    pub backup_directory: String,
    pub auto_backup: bool,
    pub backup_before_launch: bool,
    pub backup_compression_enabled: bool,
    pub backup_compression_level: i32,
    pub backup_skip_compression_once: bool,
    pub max_backups_per_game: i32,
    pub rawg_api_key: String,
    pub start_minimized_in_tray: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            ludusavi_path: String::new(),
            backup_directory: String::new(),
            auto_backup: true,
            backup_before_launch: false,
            backup_compression_enabled: true,
            backup_compression_level: 60,
            backup_skip_compression_once: false,
            max_backups_per_game: 5,
            rawg_api_key: String::new(),
            start_minimized_in_tray: false,
        }
    }
}

fn clamp_max_backups(value: i32) -> i32 {
    value.clamp(1, 100)
}

fn clamp_compression_level(value: i32) -> i32 {
    value.clamp(1, 100)
}

#[tauri::command]
pub fn get_all_settings() -> Result<AppSettings, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let mut rows = stmt.query([])?;

        let mut settings = AppSettings::default();

        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;

            match key.as_str() {
                "theme" => settings.theme = value,
                "ludusavi_path" => settings.ludusavi_path = value,
                "backup_directory" => settings.backup_directory = value,
                "auto_backup" => settings.auto_backup = value == "true",
                "backup_before_launch" => settings.backup_before_launch = value == "true",
                "backup_compression_enabled" => {
                    settings.backup_compression_enabled = value == "true"
                }
                "backup_compression_level" => {
                    settings.backup_compression_level =
                        clamp_compression_level(value.parse().unwrap_or(60))
                }
                "backup_skip_compression_once" => {
                    settings.backup_skip_compression_once = value == "true"
                }
                "max_backups_per_game" => {
                    settings.max_backups_per_game = clamp_max_backups(value.parse().unwrap_or(5))
                }
                "rawg_api_key" => settings.rawg_api_key = value,
                "start_minimized_in_tray" => {
                    settings.start_minimized_in_tray = value == "true"
                }
                _ => {}
            }
        }

        Ok(settings)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_settings(settings: AppSettings) -> Result<(), String> {
    with_db_mut(|conn| {
        let max_backups = clamp_max_backups(settings.max_backups_per_game);
        let compression_level = clamp_compression_level(settings.backup_compression_level);
        let pairs = vec![
            ("theme", settings.theme),
            ("ludusavi_path", settings.ludusavi_path),
            ("backup_directory", settings.backup_directory),
            (
                "auto_backup",
                if settings.auto_backup {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            ),
            (
                "backup_before_launch",
                if settings.backup_before_launch {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            ),
            (
                "backup_compression_enabled",
                if settings.backup_compression_enabled {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            ),
            ("backup_compression_level", compression_level.to_string()),
            (
                "backup_skip_compression_once",
                if settings.backup_skip_compression_once {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            ),
            ("max_backups_per_game", max_backups.to_string()),
            ("rawg_api_key", settings.rawg_api_key),
            (
                "start_minimized_in_tray",
                if settings.start_minimized_in_tray {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            ),
        ];

        let tx = conn.transaction()?;
        for (key, value) in pairs {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        tx.commit()?;

        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_setting(key: String) -> Result<Option<String>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let value = stmt.query_row(params![key], |row| row.get(0)).ok();
        Ok(value)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(key: String, value: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_scan_directory(path: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO scan_directories (path) VALUES (?1)",
            params![path],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_scan_directories() -> Result<Vec<String>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT path FROM scan_directories")?;
        let paths = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(paths)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_scan_directory(path: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "DELETE FROM scan_directories WHERE path = ?1",
            params![path],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

pub fn should_start_minimized_in_tray() -> bool {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT value FROM settings WHERE key = ?1",
        )?;
        let value: Option<String> = stmt
            .query_row(params!["start_minimized_in_tray"], |row| row.get(0))
            .optional()?;
        Ok(value.as_deref() == Some("true"))
    })
    .unwrap_or(false)
}
