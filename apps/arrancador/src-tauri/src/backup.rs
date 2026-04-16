use crate::database::{with_db, with_db_mut};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::ffi::OsStr;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;
use uuid::Uuid;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
    IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::IO::DeviceIoControl;

// Import our new native engine
#[path = "backup/engine.rs"]
pub mod engine;
#[path = "backup/save_locator.rs"]
pub mod save_locator;
#[path = "backup/sqoba_manifest.rs"]
pub mod sqoba_manifest;
use engine::{
    load_backup_manifest, BackupArchiveManifest, BackupEngine, BackupOptions, BackupProgress,
};

lazy_static::lazy_static! {
    static ref BACKUP_ENGINE: Mutex<BackupEngine> = Mutex::new(BackupEngine::new());
}

const SAVE_PATH_GAME_TOKEN: &str = "{PATHTOGAME}";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub id: String,
    pub game_id: String,
    pub backup_path: String,
    pub backup_size: i64,
    pub created_at: String,
    pub is_auto: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub game_name: String,
    pub save_path: Option<String>,
    pub registry_path: Option<String>,
    pub total_size: u64,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavePathLookup {
    pub save_path: Option<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupProgressEvent {
    pub game_id: String,
    pub stage: String,
    pub message: String,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreCheck {
    pub should_restore: bool,
    pub backup_id: Option<String>,
    pub current_size: u64,
    pub backup_size: i64,
}

pub(crate) fn get_backup_directory() -> PathBuf {
    let custom_path: String = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'backup_directory'")?;
        let result: String = stmt.query_row([], |row| row.get(0)).unwrap_or_default();
        Ok(result)
    })
    .unwrap_or_default();

    if !custom_path.is_empty() {
        return PathBuf::from(custom_path);
    }

    // Default backup directory
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("arrancador").join("backups")
}

fn sanitize_folder_name(name: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut cleaned: String = name.chars().filter(|c| !invalid.contains(c)).collect();
    cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "game".to_string()
    } else {
        cleaned
    }
}

fn remove_backup_artifact(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn build_unique_backup_path(
    game_backup_dir: &Path,
    timestamp: &str,
    use_compression: bool,
) -> PathBuf {
    let mut attempt = 0usize;

    loop {
        let suffix = if attempt == 0 {
            timestamp.to_string()
        } else {
            format!("{}_{}", timestamp, attempt)
        };
        let candidate = if use_compression {
            game_backup_dir.join(format!("{}.sqoba.zip", suffix))
        } else {
            game_backup_dir.join(&suffix)
        };

        if !candidate.exists() {
            return candidate;
        }

        attempt += 1;
    }
}

#[derive(Debug, Clone)]
pub struct BackupImportResult {
    #[allow(dead_code)]
    pub backups_added: usize,
    #[allow(dead_code)]
    pub save_path: Option<String>,
}

#[derive(Debug, Clone)]
struct BackupImportEntry {
    path: PathBuf,
    size: u64,
    created_at: DateTime<Utc>,
    save_root: Option<String>,
}

pub fn import_existing_backups_for_game(
    game_id: &str,
    game_name: &str,
) -> Result<BackupImportResult, String> {
    let backup_root = get_backup_directory();
    if !backup_root.exists() {
        return Ok(BackupImportResult {
            backups_added: 0,
            save_path: None,
        });
    }

    let year = get_game_year(game_id);
    let candidate_dirs = find_backup_game_dirs(&backup_root, game_name, year.as_deref());
    if candidate_dirs.is_empty() {
        return Ok(BackupImportResult {
            backups_added: 0,
            save_path: None,
        });
    }

    let mut entries: Vec<BackupImportEntry> = Vec::new();
    let mut seen_paths = HashMap::new();

    for dir in candidate_dirs {
        if let Ok(dir_entries) = fs::read_dir(&dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    if path.extension().and_then(|s| s.to_str()).is_none() {
                        continue;
                    }
                    let lower = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if lower != "zip" {
                        continue;
                    }
                }

                if seen_paths.contains_key(&path) {
                    continue;
                }

                if let Ok(Some(manifest)) = load_backup_manifest(&path) {
                    let size = manifest.files.iter().map(|f| f.size).sum();
                    let created_at = backup_entry_timestamp(&path);
                    let save_root = derive_save_root_from_manifest(&manifest);
                    entries.push(BackupImportEntry {
                        path: path.clone(),
                        size,
                        created_at,
                        save_root,
                    });
                    seen_paths.insert(path, true);
                }
            }
        }
    }

    if entries.is_empty() {
        return Ok(BackupImportResult {
            backups_added: 0,
            save_path: None,
        });
    }

    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let save_path = entries.iter().find_map(|entry| entry.save_root.clone());
    let last_backup = entries.first().map(|entry| entry.created_at.to_rfc3339());
    let backup_count = entries.len() as i32;

    with_db(|conn| {
        for entry in &entries {
            let backup_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO backups (id, game_id, backup_path, backup_size, created_at, is_auto, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, NULL)",
                params![
                    backup_id,
                    game_id,
                    entry.path.to_string_lossy().to_string(),
                    entry.size as i64,
                    entry.created_at.to_rfc3339()
                ],
            )?;
        }

        conn.execute(
            "UPDATE games SET backup_count = ?1, last_backup = ?2, backup_enabled = 1 WHERE id = ?3",
            params![backup_count, last_backup, game_id],
        )?;

        if let Some(path) = &save_path {
            let path = tokenise_save_path(game_id, path);
            conn.execute(
                "UPDATE games SET save_path = ?1, save_path_checked = 1 WHERE id = ?2",
                params![path, game_id],
            )?;
        }

        Ok(())
    })
    .map_err(|e| e.to_string())?;

    Ok(BackupImportResult {
        backups_added: entries.len(),
        save_path,
    })
}

fn find_backup_game_dirs(backup_root: &Path, game_name: &str, year: Option<&str>) -> Vec<PathBuf> {
    let base = sanitize_folder_name(game_name).to_lowercase();
    if base.is_empty() {
        return Vec::new();
    }
    let expected_parens = year
        .map(|y| y.trim())
        .filter(|y| !y.is_empty())
        .map(|y| format!("{} ({})", base, y.to_lowercase()));
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(backup_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let matches_default = name == base || name.starts_with(&format!("{}-", base));
            let matches_parens = expected_parens
                .as_ref()
                .map(|expected| name == *expected)
                .unwrap_or(false);
            if matches_default || matches_parens {
                out.push(path);
            }
        }
    }
    out
}

fn backup_entry_timestamp(path: &Path) -> DateTime<Utc> {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if let Some(dt) = parse_backup_timestamp(name) {
        return dt;
    }
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            return DateTime::<Utc>::from(modified);
        }
    }
    Utc::now()
}

fn parse_backup_timestamp(name: &str) -> Option<DateTime<Utc>> {
    let trimmed = name
        .strip_suffix(".sqoba.zip")
        .or_else(|| name.strip_suffix(".zip"))
        .unwrap_or(name);
    let naive = NaiveDateTime::parse_from_str(trimmed, "%H%M%S_%d%m%Y").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
}

fn derive_save_root_from_manifest(manifest: &BackupArchiveManifest) -> Option<String> {
    let mut totals: HashMap<String, u64> = HashMap::new();
    let mut roots: HashMap<String, PathBuf> = HashMap::new();

    for entry in &manifest.files {
        let (root_label, rel) = match parse_backup_relative(&entry.backup_path) {
            Some(value) => value,
            None => continue,
        };
        let original_path = PathBuf::from(&entry.original_path);
        let root = strip_suffix_path(&original_path, &rel)
            .or_else(|| original_path.parent().map(|p| p.to_path_buf()));
        let Some(root) = root else {
            continue;
        };

        *totals.entry(root_label.clone()).or_insert(0) += entry.size;
        roots.entry(root_label).or_insert(root);
    }

    if totals.is_empty() {
        if let Some(first) = manifest.files.first() {
            return PathBuf::from(&first.original_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string());
        }
        return None;
    }

    let best_label = totals
        .iter()
        .max_by_key(|(_, size)| *size)
        .map(|(label, _)| label.clone())?;
    roots
        .get(&best_label)
        .map(|path| path.to_string_lossy().to_string())
}

fn parse_backup_relative(backup_path: &str) -> Option<(String, PathBuf)> {
    let parts: Vec<&str> = backup_path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 || parts[0] != "files" {
        return None;
    }
    let root_label = parts[1].to_string();
    let mut rel = PathBuf::new();
    for part in parts.iter().skip(2) {
        rel.push(part);
    }
    Some((root_label, rel))
}

fn strip_suffix_path(path: &Path, suffix: &Path) -> Option<PathBuf> {
    let path_components: Vec<_> = path.components().collect();
    let suffix_components: Vec<_> = suffix.components().collect();
    if suffix_components.is_empty() {
        return Some(path.to_path_buf());
    }
    if path_components.len() < suffix_components.len() {
        return None;
    }
    let start = path_components.len() - suffix_components.len();
    for (a, b) in path_components[start..]
        .iter()
        .zip(suffix_components.iter())
    {
        let a_str = a.as_os_str().to_string_lossy();
        let b_str = b.as_os_str().to_string_lossy();
        let matches = if cfg!(target_os = "windows") {
            a_str.to_lowercase() == b_str.to_lowercase()
        } else {
            a_str == b_str
        };
        if !matches {
            return None;
        }
    }
    let mut out = PathBuf::new();
    for comp in path_components.iter().take(start) {
        out.push(comp.as_os_str());
    }
    Some(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiskType {
    Hdd,
    Ssd,
    Unknown,
}

fn get_drive_letter(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        return Some(s[0..2].to_string());
    }
    None
}

fn load_disk_type(letter: &str) -> Option<DiskType> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let key = format!("disk_type_{}", letter);
        let value: Option<String> = stmt.query_row(params![key], |row| row.get(0)).ok();
        Ok(value)
    })
    .ok()
    .flatten()
    .map(|v| match v.as_str() {
        "hdd" => DiskType::Hdd,
        "ssd" => DiskType::Ssd,
        _ => DiskType::Unknown,
    })
}

fn save_disk_type(letter: &str, disk_type: DiskType) {
    let value = match disk_type {
        DiskType::Hdd => "hdd",
        DiskType::Ssd => "ssd",
        DiskType::Unknown => "unknown",
    };
    let key = format!("disk_type_{}", letter);
    let _ = with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    });
}

#[cfg(target_os = "windows")]
fn detect_disk_type_windows(path: &Path) -> DiskType {
    let letter = match get_drive_letter(path) {
        Some(l) => l,
        None => return DiskType::Unknown,
    };
    if let Some(cached) = load_disk_type(&letter) {
        return cached;
    }
    let device = format!("\\\\.\\{}", letter);
    let wide: Vec<u16> = OsStr::new(&device)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )
    };
    let handle = match handle {
        Ok(h) => h,
        Err(_) => return DiskType::Unknown,
    };
    if handle.is_invalid() {
        return DiskType::Unknown;
    }

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceSeekPenaltyProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut desc = DEVICE_SEEK_PENALTY_DESCRIPTOR {
        Version: 0,
        Size: std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
        IncursSeekPenalty: false.into(),
    };
    let mut bytes_returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as _),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(&mut desc as *mut _ as _),
            std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .is_ok()
    };
    unsafe {
        let _ = CloseHandle(handle);
    }
    let disk_type = if ok && desc.IncursSeekPenalty.as_bool() {
        DiskType::Hdd
    } else if ok {
        DiskType::Ssd
    } else {
        DiskType::Unknown
    };
    save_disk_type(&letter, disk_type);
    disk_type
}

#[cfg(not(target_os = "windows"))]
fn detect_disk_type_windows(_path: &Path) -> DiskType {
    DiskType::Unknown
}

fn get_disk_threads(path: &Path) -> usize {
    let cpu_count = num_cpus::get().max(1);
    match detect_disk_type_windows(path) {
        DiskType::Hdd => 2.min(cpu_count),
        DiskType::Ssd => 8.min(cpu_count),
        DiskType::Unknown => 4.min(cpu_count),
    }
}

fn get_game_year(game_id: &str) -> Option<String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT released FROM games WHERE id = ?1")?;
        let released: Option<String> = stmt.query_row(params![game_id], |row| row.get(0)).ok();
        Ok(released)
    })
    .ok()
    .flatten()
    .and_then(|r| r.split('-').next().map(|s| s.to_string()))
}

fn get_setting_value(key: &str) -> Option<String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        Ok(stmt.query_row(params![key], |row| row.get(0)).ok())
    })
    .ok()
    .flatten()
}

fn get_setting_bool(key: &str, default: bool) -> bool {
    get_setting_value(key)
        .map(|value| value == "true")
        .unwrap_or(default)
}

fn get_setting_i32(key: &str, default: i32) -> i32 {
    get_setting_value(key)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
}

fn get_game_exe_path(game_id: &str) -> Option<String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT exe_path FROM games WHERE id = ?1")?;
        let value: Option<String> = stmt
            .query_row(params![game_id], |row| row.get(0))
            .unwrap_or(None);
        Ok(value)
    })
    .ok()
    .flatten()
    .and_then(|path| {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn resolve_save_path(game_id: &str, raw: &str) -> Result<String, String> {
    if !raw.contains(SAVE_PATH_GAME_TOKEN) {
        return Ok(raw.to_string());
    }
    let exe_path = get_game_exe_path(game_id)
        .ok_or_else(|| "Не удалось подставить {PATHTOGAME}: путь к игре не найден".to_string())?;
    let exe_dir = Path::new(&exe_path).parent().ok_or_else(|| {
        "Не удалось подставить {PATHTOGAME}: у пути к игре нет родительской папки".to_string()
    })?;
    Ok(raw.replace(SAVE_PATH_GAME_TOKEN, &exe_dir.to_string_lossy()))
}

fn tokenise_save_path(game_id: &str, save_path: &str) -> String {
    if save_path.contains(SAVE_PATH_GAME_TOKEN) {
        return save_path.to_string();
    }

    let save_path_pb = PathBuf::from(save_path);
    if !save_path_pb.is_absolute() || !save_path_pb.exists() {
        return save_path.to_string();
    }

    let Some(exe_path) = get_game_exe_path(game_id) else {
        return save_path.to_string();
    };
    let Some(game_dir) = Path::new(&exe_path).parent() else {
        return save_path.to_string();
    };

    let Ok(game_dir) = fs::canonicalize(game_dir) else {
        return save_path.to_string();
    };
    let Ok(save_path_pb) = fs::canonicalize(save_path_pb) else {
        return save_path.to_string();
    };

    let Ok(relative) = save_path_pb.strip_prefix(&game_dir) else {
        return save_path.to_string();
    };

    if relative.as_os_str().is_empty() {
        return SAVE_PATH_GAME_TOKEN.to_string();
    }

    let mut out = String::from(SAVE_PATH_GAME_TOKEN);
    out.push(std::path::MAIN_SEPARATOR);
    out.push_str(&relative.to_string_lossy());
    out
}

fn get_game_save_path(game_id: &str) -> Result<Option<String>, String> {
    let raw: Option<String> = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT save_path FROM games WHERE id = ?1")?;
        let value: Option<String> = stmt
            .query_row(params![game_id], |row| row.get(0))
            .unwrap_or(None);
        Ok(value)
    })
    .map_err(|e| e.to_string())?;

    let Some(path) = raw else {
        return Ok(None);
    };
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(resolve_save_path(game_id, trimmed)?))
}

fn set_game_save_path(game_id: &str, save_path: &str) -> Result<(), String> {
    let save_path = tokenise_save_path(game_id, save_path);
    with_db(|conn| {
        conn.execute(
            "UPDATE games SET save_path = ?1 WHERE id = ?2",
            params![save_path, game_id],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn get_compression_settings() -> (bool, u8, bool) {
    let enabled = get_setting_bool("backup_compression_enabled", true);
    let level = get_setting_i32("backup_compression_level", 60).clamp(1, 100) as u8;
    let skip_once = get_setting_bool("backup_skip_compression_once", false);
    (enabled, level, skip_once)
}

fn clear_skip_compression_once() {
    let _ = with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('backup_skip_compression_once', 'false')",
            [],
        )?;
        Ok(())
    });
}

fn get_max_backups() -> i32 {
    get_setting_i32("max_backups_per_game", 5).clamp(1, 100)
}

// Deprecated but kept for API compatibility, always returns true now
#[tauri::command]
pub fn check_ludusavi_installed() -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub fn get_ludusavi_executable_path() -> Result<Option<String>, String> {
    Ok(Some("native".to_string()))
}

#[tauri::command]
pub fn set_ludusavi_path(_path: String) -> Result<(), String> {
    Ok(()) // No-op
}

#[tauri::command]
pub fn set_backup_directory(path: String) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|e| format!("Не удалось создать папку: {}", e))?;

    with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('backup_directory', ?1)",
            params![path],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_backup_directory_setting() -> Result<String, String> {
    Ok(get_backup_directory().to_string_lossy().to_string())
}

#[tauri::command]
pub fn refresh_sqoba_manifest() -> Result<(), String> {
    crate::backup::sqoba_manifest::refresh_manifest_from_network()
        .map_err(|e| format!("Не удалось обновить манифест SQOBA: {}", e))?;

    // Reload the in-memory cache so subsequent calls don't re-read/parse the manifest.
    let mut engine = BACKUP_ENGINE.lock().map_err(|e| e.to_string())?;
    engine
        .reload_manifest()
        .map_err(|e| format!("Не удалось перезагрузить манифест: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn find_game_save_paths(
    game_name: String,
    game_id: Option<String>,
) -> Result<SavePathLookup, String> {
    let mut engine = BACKUP_ENGINE.lock().map_err(|e| e.to_string())?;

    engine
        .load_manifest()
        .map_err(|e| format!("Не удалось загрузить манифест: {}", e))?;

    let year = game_id.as_deref().and_then(get_game_year);
    let name_with_year = year.map(|y| format!("{} ({})", game_name, y));

    let mut last_err: Option<String> = None;
    let save_override = match game_id.as_deref() {
        Some(id) => match get_game_save_path(id) {
            Ok(value) => value,
            Err(e) => {
                last_err = Some(e);
                None
            }
        },
        None => None,
    };

    let mut roots = match engine.discover_game_save_roots(&game_name, save_override.as_deref()) {
        Ok(value) => value,
        Err(e) => {
            last_err = Some(e);
            Vec::new()
        }
    };

    if roots.is_empty() && save_override.is_some() {
        roots = match engine.discover_game_save_roots(&game_name, None) {
            Ok(value) => value,
            Err(e) => {
                last_err = Some(e);
                Vec::new()
            }
        };
    }

    if roots.is_empty() {
        if let Some(alt) = &name_with_year {
            roots = match engine.discover_game_save_roots(alt, None) {
                Ok(value) => value,
                Err(e) => {
                    last_err = Some(e);
                    Vec::new()
                }
            };
        }
    }

    if roots.is_empty() {
        let _ = last_err;
        return Ok(SavePathLookup {
            save_path: None,
            candidates: Vec::new(),
        });
    }

    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in &roots {
        let value = root.path.to_string_lossy().to_string();
        if seen.insert(value.clone()) {
            candidates.push(value);
        }
    }

    let first_root = candidates.first().cloned();
    let mut save_path = save_override.clone().or_else(|| first_root.clone());

    if save_override.is_none() && candidates.len() == 1 {
        if let (Some(game_id), Some(candidate)) = (game_id.as_deref(), first_root.clone()) {
            if set_game_save_path(game_id, &candidate).is_ok() {
                save_path = Some(candidate);
            }
        }
    }

    Ok(SavePathLookup {
        save_path,
        candidates,
    })
}

#[tauri::command]
pub fn find_game_saves(
    game_name: String,
    game_id: Option<String>,
) -> Result<Option<BackupInfo>, String> {
    let mut engine = BACKUP_ENGINE.lock().map_err(|e| e.to_string())?;

    // Ensure manifest is loaded
    engine
        .load_manifest()
        .map_err(|e| format!("Не удалось загрузить манифест: {}", e))?;

    let year = game_id.as_deref().and_then(get_game_year);
    let name_with_year = year.map(|y| format!("{} ({})", game_name, y));

    let mut last_err: Option<String> = None;
    let save_override = match game_id.as_deref() {
        Some(id) => match get_game_save_path(id) {
            Ok(value) => value,
            Err(e) => {
                last_err = Some(e);
                None
            }
        },
        None => None,
    };
    let mut discovery = match engine.discover_game_saves(&game_name, save_override.as_deref()) {
        Ok(value) => value,
        Err(e) => {
            last_err = Some(e);
            None
        }
    };

    if discovery.is_none() && save_override.is_some() {
        discovery = match engine.discover_game_saves(&game_name, None) {
            Ok(value) => value,
            Err(e) => {
                last_err = Some(e);
                None
            }
        };
    }

    if discovery.is_none() {
        if let Some(alt) = &name_with_year {
            discovery = match engine.discover_game_saves(alt, None) {
                Ok(value) => value,
                Err(e) => {
                    last_err = Some(e);
                    None
                }
            };
        }
    }

    match discovery {
        Some(discovery) => {
            let file_strings: Vec<String> = discovery
                .files
                .iter()
                .map(|entry| entry.path.to_string_lossy().to_string())
                .collect();
            let first_root = discovery
                .roots
                .first()
                .map(|root| root.path.to_string_lossy().to_string());
            let mut save_path = save_override.clone().or_else(|| first_root.clone());

            if save_override.is_none() && discovery.roots.len() == 1 {
                if let (Some(game_id), Some(candidate)) = (game_id.as_deref(), first_root.clone()) {
                    if set_game_save_path(game_id, &candidate).is_ok() {
                        save_path = Some(candidate);
                    }
                }
            }

            Ok(Some(BackupInfo {
                game_name,
                save_path,
                registry_path: None,
                total_size: discovery.total_size,
                files: file_strings,
            }))
        }
        None => {
            // If we couldn't resolve an override or any heuristic, just report "not found".
            // This keeps the UI behaviour consistent and avoids surfacing low-level errors.
            let _ = last_err;
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn create_backup(
    app: tauri::AppHandle,
    game_id: String,
    game_name: String,
    is_auto: bool,
    notes: Option<String>,
) -> Result<Backup, String> {
    let game_id_clone = game_id.clone();
    let game_name_clone = game_name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        create_backup_inner(Some(app), game_id_clone, game_name_clone, is_auto, notes)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn create_backup_inner(
    app: Option<tauri::AppHandle>,
    game_id: String,
    game_name: String,
    is_auto: bool,
    notes: Option<String>,
) -> Result<Backup, String> {
    let mut engine = BACKUP_ENGINE.lock().map_err(|e| e.to_string())?;
    // Ensure manifest
    engine
        .load_manifest()
        .map_err(|e| format!("Не удалось загрузить манифест: {}", e))?;
    let save_path_override = get_game_save_path(&game_id).unwrap_or(None);

    let backup_root = get_backup_directory();
    let threads = get_disk_threads(&backup_root);
    let year = get_game_year(&game_id);
    let safe_name = sanitize_folder_name(&game_name);
    let game_folder = match year {
        Some(y) => format!("{}-{}", safe_name, y),
        None => safe_name,
    };

    let game_backup_dir = backup_root.join(game_folder);
    fs::create_dir_all(&game_backup_dir)
        .map_err(|e| format!("Не удалось создать папку для бэкапов: {}", e))?;

    // Create timestamped backup folder
    let timestamp = Local::now().format("%H%M%S_%3f_%d%m%Y").to_string();
    let (compression_enabled, compression_level, skip_once) = get_compression_settings();
    let use_compression = compression_enabled && !skip_once;
    if skip_once {
        clear_skip_compression_once();
    }
    let backup_path = build_unique_backup_path(&game_backup_dir, &timestamp, use_compression);
    let backup_options = if use_compression {
        BackupOptions::zip(compression_level)
    } else {
        BackupOptions::directory()
    };

    // Run native backup
    if let Some(app) = &app {
        let _ = app.emit(
            "backup:progress",
            BackupProgressEvent {
                game_id: game_id.clone(),
                stage: "scan".to_string(),
                message: "Сканирование файлов".to_string(),
                done: 0,
                total: 0,
            },
        );
    }

    let progress: Option<Arc<dyn Fn(BackupProgress) + Send + Sync>> = app.as_ref().map(|app| {
        let app = app.clone();
        let game_id = game_id.clone();
        Arc::new(move |p: BackupProgress| {
            let _ = app.emit(
                "backup:progress",
                BackupProgressEvent {
                    game_id: game_id.clone(),
                    stage: p.stage.to_string(),
                    message: p.current,
                    done: p.done,
                    total: p.total,
                },
            );
        }) as Arc<dyn Fn(BackupProgress) + Send + Sync>
    });

    let year = get_game_year(&game_id);
    let name_with_year = year.map(|y| format!("{} ({})", game_name, y));

    let mut attempts: Vec<(&str, Option<&str>)> = Vec::new();
    attempts.push((&game_name, save_path_override.as_deref()));
    if save_path_override.is_some() {
        attempts.push((&game_name, None));
    }
    if let Some(alt) = name_with_year.as_deref() {
        attempts.push((alt, None));
    }

    let mut last_err: Option<String> = None;
    let mut backup_size: Option<u64> = None;
    for (name, override_path) in attempts {
        match engine.backup_game_with_options_and_progress(
            name,
            &backup_path,
            threads,
            backup_options,
            override_path,
            progress.clone(),
        ) {
            Ok(size) => {
                backup_size = Some(size);
                break;
            }
            Err(e) => {
                last_err = Some(e);
                let _ = remove_backup_artifact(&backup_path);
            }
        }
    }

    let Some(backup_size) = backup_size else {
        return Err(last_err.unwrap_or_else(|| "Не удалось создать бэкап".to_string()));
    };

    let progress_app = app.as_ref().cloned();

    if backup_size == 0 {
        remove_backup_artifact(&backup_path)?;
        return Err("No save data found for this game".to_string());
    }

    // Record backup in database
    let backup_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    with_db(|conn| {
        conn.execute(
            "INSERT INTO backups (id, game_id, backup_path, backup_size, created_at, is_auto, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                backup_id,
                game_id,
                backup_path.to_string_lossy().to_string(),
                backup_size as i64,
                created_at,
                if is_auto { 1 } else { 0 },
                notes
            ],
        )?;

        // Update game backup info
        conn.execute(
            "UPDATE games SET last_backup = ?1, backup_count = backup_count + 1, backup_enabled = 1 WHERE id = ?2",
            params![created_at, game_id],
        )?;

        Ok(())
    })
    .map_err(|e| e.to_string())?;

    // Cleanup old backups
    cleanup_old_backups(&game_id)?;

    if let Some(app) = &progress_app {
        let _ = app.emit(
            "backup:progress",
            BackupProgressEvent {
                game_id: game_id.clone(),
                stage: "done".to_string(),
                message: "Backup created".to_string(),
                done: 0,
                total: 0,
            },
        );
    }

    Ok(Backup {
        id: backup_id,
        game_id,
        backup_path: backup_path.to_string_lossy().to_string(),
        backup_size: backup_size as i64,
        created_at,
        is_auto,
        notes,
    })
}

fn cleanup_old_backups(game_id: &str) -> Result<(), String> {
    let max_backups = get_max_backups();

    let backups: Vec<Backup> = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
             FROM backups WHERE game_id = ?1 ORDER BY created_at DESC",
        )?;

        let backups = stmt
            .query_map(params![game_id], |row| {
                Ok(Backup {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    backup_path: row.get(2)?,
                    backup_size: row.get(3)?,
                    created_at: row.get(4)?,
                    is_auto: row.get::<_, i32>(5)? == 1,
                    notes: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(backups)
    })
    .map_err(|e: rusqlite::Error| e.to_string())?;

    // Keep only max_backups, delete the rest
    if backups.len() > max_backups as usize {
        let backups_to_delete: Vec<Backup> =
            backups.into_iter().skip(max_backups as usize).collect();
        with_db_mut(|conn| {
            let tx = conn.transaction()?;
            for backup in &backups_to_delete {
                tx.execute("DELETE FROM backups WHERE id = ?1", params![&backup.id])?;
            }
            tx.execute(
                "UPDATE games SET backup_count = ?1 WHERE id = ?2",
                params![max_backups, game_id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

        for backup in backups_to_delete {
            remove_backup_artifact(Path::new(&backup.backup_path))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_game_backups(game_id: String) -> Result<Vec<Backup>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
             FROM backups WHERE game_id = ?1 ORDER BY created_at DESC",
        )?;

        let backups = stmt
            .query_map(params![game_id], |row| {
                Ok(Backup {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    backup_path: row.get(2)?,
                    backup_size: row.get(3)?,
                    created_at: row.get(4)?,
                    is_auto: row.get::<_, i32>(5)? == 1,
                    notes: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(backups)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_backup(app: tauri::AppHandle, backup_id: String) -> Result<(), String> {
    let backup: Backup = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
             FROM backups WHERE id = ?1",
        )?;

        stmt.query_row(params![backup_id], |row| {
            Ok(Backup {
                id: row.get(0)?,
                game_id: row.get(1)?,
                backup_path: row.get(2)?,
                backup_size: row.get(3)?,
                created_at: row.get(4)?,
                is_auto: row.get::<_, i32>(5)? == 1,
                notes: row.get(6)?,
            })
        })
    })
    .map_err(|e| e.to_string())?;

    let backup_path = backup.backup_path.clone();
    let game_id = backup.game_id.clone();
    let _ = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT name FROM games WHERE id = ?1")?;
        let name = stmt.query_row(params![game_id], |row| {
            let value: Option<String> = row.get(0)?;
            Ok(value)
        });
        name
    })
    .map_err(|e| format!("Failed to load game name: {}", e))?
    .unwrap_or_default();
    let threads = get_disk_threads(Path::new(&backup_path));
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let engine = BACKUP_ENGINE.lock().map_err(|e| e.to_string())?;
        let progress: Arc<dyn Fn(BackupProgress) + Send + Sync> = {
            let app = app.clone();
            let game_id = game_id.clone();
            Arc::new(move |p: BackupProgress| {
                let _ = app.emit(
                    "restore:progress",
                    BackupProgressEvent {
                        game_id: game_id.clone(),
                        stage: p.stage.to_string(),
                        message: p.current,
                        done: p.done,
                        total: p.total,
                    },
                );
            }) as Arc<dyn Fn(BackupProgress) + Send + Sync>
        };
        let result = engine.restore_backup_with_threads_and_progress(
            Path::new(&backup_path),
            threads,
            Some(progress),
        );
        if result.is_ok() {
        let _ = app.emit(
            "restore:progress",
            BackupProgressEvent {
                game_id: game_id.clone(),
                stage: "done".to_string(),
                message: "Восстановление завершено".to_string(),
                done: 0,
                total: 0,
            },
        );
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_backup(backup_id: String) -> Result<(), String> {
    let backup: Backup = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
             FROM backups WHERE id = ?1",
        )?;

        stmt.query_row(params![backup_id], |row| {
            Ok(Backup {
                id: row.get(0)?,
                game_id: row.get(1)?,
                backup_path: row.get(2)?,
                backup_size: row.get(3)?,
                created_at: row.get(4)?,
                is_auto: row.get::<_, i32>(5)? == 1,
                notes: row.get(6)?,
            })
        })
    })
    .map_err(|e| e.to_string())?;

    // Delete backup path
    let backup_path = Path::new(&backup.backup_path);
    if backup_path.exists() {
        if backup_path.is_dir() {
            fs::remove_dir_all(backup_path)
                .map_err(|e| format!("Не удалось удалить папку бэкапа: {}", e))?;
        } else {
            fs::remove_file(backup_path)
                .map_err(|e| format!("Не удалось удалить файл бэкапа: {}", e))?;
        }
    }

    // Remove from database
    with_db(|conn| {
        conn.execute("DELETE FROM backups WHERE id = ?1", params![backup_id])?;
        conn.execute(
            "UPDATE games SET backup_count = backup_count - 1 WHERE id = ?1 AND backup_count > 0",
            params![backup.game_id],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn should_backup_before_launch(game_id: String) -> Result<bool, String> {
    // Check if auto backup is enabled globally
    let auto_backup: String = with_db(|conn| {
        let mut stmt =
            conn.prepare("SELECT value FROM settings WHERE key = 'backup_before_launch'")?;
        let result: String = stmt
            .query_row([], |row| row.get(0))
            .unwrap_or_else(|_| "false".to_string());
        Ok(result)
    })
    .unwrap_or_else(|_| "false".to_string());

    if auto_backup != "true" {
        return Ok(false);
    }

    // Check if game has backup enabled
    let backup_enabled: bool = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT backup_enabled FROM games WHERE id = ?1")?;
        let enabled: i32 = stmt
            .query_row(params![game_id], |row| row.get(0))
            .unwrap_or(0);
        Ok(enabled == 1)
    })
    .unwrap_or(false);

    Ok(backup_enabled)
}

#[tauri::command]
pub fn check_backup_needed(game_id: String, game_name: String) -> Result<bool, String> {
    // Find current save data
    let save_info = find_game_saves(game_name, Some(game_id.clone()))?;

    if save_info.is_none() {
        return Ok(false);
    }

    let save_info = save_info.unwrap();

    // Get last backup
    let last_backup: Option<Backup> = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
             FROM backups WHERE game_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;

        let backup = stmt
            .query_row(params![game_id], |row| {
                Ok(Backup {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    backup_path: row.get(2)?,
                    backup_size: row.get(3)?,
                    created_at: row.get(4)?,
                    is_auto: row.get::<_, i32>(5)? == 1,
                    notes: row.get(6)?,
                })
            })
            .ok();
        Ok(backup)
    })
    .ok()
    .flatten();

    match last_backup {
        None => Ok(true), // No backup exists, should create one
        Some(backup) => {
            // Check if save data is newer and larger than backup
            let _current_size = save_info.total_size as i64;

            // If current save is larger, we should backup
            // Note: ZIP compression makes this check unreliable if we compare compressed vs raw
            // So we just check time mostly.

            // Check modification time of save files
            for file_path in &save_info.files {
                if let Ok(metadata) = fs::metadata(file_path) {
                    if let Ok(modified) = metadata.modified() {
                        let file_time: DateTime<Utc> = modified.into();
                        if let Ok(backup_time) = DateTime::parse_from_rfc3339(&backup.created_at) {
                            if file_time > backup_time.with_timezone(&Utc) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }

            Ok(false)
        }
    }
}

#[tauri::command]
pub fn check_restore_needed(game_id: String, game_name: String) -> Result<RestoreCheck, String> {
    let save_info = find_game_saves(game_name, Some(game_id.clone()))?;

    if save_info.is_none() {
        return Ok(RestoreCheck {
            should_restore: false,
            backup_id: None,
            current_size: 0,
            backup_size: 0,
        });
    }

    let save_info = save_info.unwrap();

    let last_backup: Option<Backup> = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
             FROM backups WHERE game_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;

        let backup = stmt
            .query_row(params![game_id], |row| {
                Ok(Backup {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    backup_path: row.get(2)?,
                    backup_size: row.get(3)?,
                    created_at: row.get(4)?,
                    is_auto: row.get::<_, i32>(5)? == 1,
                    notes: row.get(6)?,
                })
            })
            .ok();
        Ok(backup)
    })
    .ok()
    .flatten();

    match last_backup {
        None => Ok(RestoreCheck {
            should_restore: false,
            backup_id: None,
            current_size: save_info.total_size,
            backup_size: 0,
        }),
        Some(backup) => {
            let should_restore = save_info.total_size < backup.backup_size as u64;
            Ok(RestoreCheck {
                should_restore,
                backup_id: Some(backup.id),
                current_size: save_info.total_size,
                backup_size: backup.backup_size,
            })
        }
    }
}

#[derive(Debug, Serialize, Clone)]
struct SavePathMissingEvent {
    game_id: String,
    game_name: String,
}

#[derive(Debug)]
struct GameExitState {
    name: String,
    backup_enabled: bool,
    save_path: Option<String>,
    save_path_checked: bool,
}

fn load_game_exit_state(game_id: &str) -> Result<GameExitState, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT name, backup_enabled, save_path, save_path_checked FROM games WHERE id = ?1",
        )?;
        let result = stmt.query_row(params![game_id], |row| {
            let name: String = row.get(0)?;
            let enabled: i32 = row.get(1)?;
            let save_path: Option<String> = row.get(2)?;
            let checked: Option<i32> = row.get(3).ok();
            Ok(GameExitState {
                name,
                backup_enabled: enabled == 1,
                save_path,
                save_path_checked: checked.unwrap_or(0) == 1,
            })
        });
        Ok(result.ok())
    })
    .unwrap_or(None)
    .ok_or_else(|| "Game not found".to_string())
}

fn set_save_path_checked(game_id: &str, checked: bool) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE games SET save_path_checked = ?1 WHERE id = ?2",
            params![if checked { 1 } else { 0 }, game_id],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn try_auto_discover_save_path(game_id: &str, game_name: &str) -> Result<bool, String> {
    let result = find_game_saves(game_name.to_string(), Some(game_id.to_string()));
    match result {
        Ok(Some(_)) => {
            set_save_path_checked(game_id, true)?;
            Ok(true)
        }
        Ok(None) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn auto_backup_on_exit(game_id: &str, app: Option<tauri::AppHandle>) -> Result<(), String> {
    let state = load_game_exit_state(game_id)?;
    if state.save_path.is_none() && !state.save_path_checked {
        match try_auto_discover_save_path(game_id, &state.name) {
            Ok(found) => {
                if !found {
                    if let Some(app) = app {
                        let _ = app.emit(
                            "game:save-path-missing",
                            SavePathMissingEvent {
                                game_id: game_id.to_string(),
                                game_name: state.name.clone(),
                            },
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Auto save discovery failed for {}: {}", game_id, e);
            }
        }
    }

    let auto_backup: String = with_db(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'auto_backup'")?;
        let result: String = stmt
            .query_row([], |row| row.get(0))
            .unwrap_or_else(|_| "true".to_string());
        Ok(result)
    })
    .unwrap_or_else(|_| "true".to_string());

    if auto_backup != "true" {
        return Ok(());
    }

    if !state.backup_enabled {
        return Ok(());
    }

    if !check_backup_needed(game_id.to_string(), state.name.clone())? {
        return Ok(());
    }

    create_backup_inner(
        None,
        game_id.to_string(),
        state.name,
        true,
        Some("Auto backup after exit".to_string()),
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_backup_settings() -> Result<serde_json::Value, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT key, value FROM settings WHERE key LIKE 'backup%' OR key = 'ludusavi_path' OR key = 'max_backups_per_game'")?;

        let mut settings = serde_json::Map::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            settings.insert(key, serde_json::Value::String(value));
        }

        Ok(serde_json::Value::Object(settings))
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_backup_settings(settings: serde_json::Value) -> Result<(), String> {
    let obj = settings.as_object().ok_or("Settings must be an object")?;

    with_db(|conn| {
        for (key, value) in obj {
            if let Some(val_str) = value.as_str() {
                conn.execute(
                    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                    params![key, val_str],
                )?;
            }
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}
