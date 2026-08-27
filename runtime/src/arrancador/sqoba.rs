// Arrancador SQOBA — Save-game Quick On-disk Backup Archive.
//
// Discovers save paths (heuristics + manual override), backs them up into a zip
// archive под `<sqoba_dest_dir>/<game_id>/<iso_timestamp>.zip`, lists existing
// backups for a game и restores files into their original paths.
//
// Метаданные source paths сохраняются внутри zip как `_sqoba_meta.json`, чтобы
// restore мог разложить файлы обратно даже если save_paths config поменялся.
//
// Rotation: после каждого `backup()` оставляем `keep_backups` (default 10)
// самых свежих zip'ов на игру; остальные удаляются.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

use crate::arrancador::config as ar_config;

const DEFAULT_KEEP_BACKUPS: u32 = 10;
const META_FILENAME: &str = "_sqoba_meta.json";
const SQOBA_FORMAT_VERSION: u32 = 2;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_SOURCE_ROOTS: usize = 64;
const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_META_BYTES: u64 = 1024 * 1024;
const RECOVERY_DIR: &str = ".recovery";
const LOCK_FILENAME: &str = ".sqoba.lock";
const JOURNAL_SUFFIX: &str = ".journal.json";

#[derive(Debug, Error)]
pub enum SqobaError {
    #[error("no save paths found for game {game_id}")]
    NoSavePathsFound { game_id: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("backup not found: {0}")]
    BackupNotFound(String),
    #[error("invalid SQOBA archive: {0}")]
    InvalidArchive(String),
    #[error("SQOBA is busy for this game")]
    Busy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqobaBackup {
    pub id: String,
    pub game_id: String,
    pub timestamp: String,
    pub dest_path: PathBuf,
    pub files_count: u32,
    pub bytes: u64,
    pub source_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub status: RestoreStatus,
    pub restored_files: u32,
    pub bytes: u64,
    pub errors: Vec<String>,
    pub recovery_backup: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestoreStatus {
    Committed,
    RolledBack,
    ManualRecoveryRequired,
}

/// Метаданные, хранящиеся внутри zip как `_sqoba_meta.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZipMeta {
    format_version: u32,
    id: String,
    game_id: String,
    timestamp: String,
    source_root_count: u32,
    files_count: u32,
    bytes: u64,
    /// Mapping: per-source index (`source_0/...` inside zip) -> absolute path.
    source_paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct ArchiveEntry {
    index: usize,
    member: String,
    target: PathBuf,
    size: u64,
}

#[derive(Debug)]
struct ArchivePlan {
    meta: ZipMeta,
    entries: Vec<ArchiveEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RestoreJournal {
    version: u32,
    game_id: String,
    backup_id: String,
    backup_path: PathBuf,
    snapshot_path: PathBuf,
    staging_dir: PathBuf,
    entries: Vec<JournalEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalEntry {
    target: PathBuf,
    snapshot_member: String,
    was_present: bool,
}

struct GameLock {
    path: PathBuf,
    _file: std::fs::File,
}

impl Drop for GameLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// ───────────────────────── path discovery ─────────────────────────

/// Возвращает существующие save-paths для игры. Если `manual_override` задан и
/// непустой — возвращаем только те его entries, что реально существуют. Иначе
/// пробегаем по стандартным эвристическим путям.
pub fn discover_save_paths(game_name: &str, manual_override: Option<&[PathBuf]>) -> Vec<PathBuf> {
    if let Some(paths) = manual_override {
        if !paths.is_empty() {
            return paths.iter().filter(|p| p.exists()).cloned().collect();
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let base = PathBuf::from(&profile);
        candidates.push(base.join("Saved Games").join(game_name));
        candidates.push(base.join("Documents").join("My Games").join(game_name));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(&local).join(game_name));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(&appdata).join(game_name));
    }

    candidates.into_iter().filter(|p| p.exists()).collect()
}

/// Корневая директория для SQOBA backup'ов. По умолчанию
/// `<data_dir>/sqoba/`; override через ArrancadorConfig.sqoba_dest_dir.
pub fn dest_root() -> PathBuf {
    let cfg = ar_config::load();
    cfg.sqoba_dest_dir
        .unwrap_or_else(|| ar_config::data_dir().join("sqoba"))
}

fn keep_count() -> u32 {
    ar_config::load()
        .keep_backups
        .unwrap_or(DEFAULT_KEEP_BACKUPS)
}

// ───────────────────────── backup ─────────────────────────

pub fn backup(
    game_id: &str,
    game_name: &str,
    manual_paths: Option<&[PathBuf]>,
) -> Result<SqobaBackup, SqobaError> {
    backup_with_root(game_id, game_name, manual_paths, &dest_root(), keep_count())
}

/// Test-friendly вариант с явным корнем (вместо чтения config).
pub fn backup_with_root(
    game_id: &str,
    game_name: &str,
    manual_paths: Option<&[PathBuf]>,
    dest_root: &Path,
    keep: u32,
) -> Result<SqobaBackup, SqobaError> {
    validate_game_id(game_id)?;
    let discovered = discover_save_paths(game_name, manual_paths);
    if discovered.is_empty() {
        return Err(SqobaError::NoSavePathsFound {
            game_id: game_id.to_string(),
        });
    }
    let sources = validate_source_roots(&discovered)?;
    if sources.is_empty() {
        return Err(SqobaError::NoSavePathsFound {
            game_id: game_id.to_string(),
        });
    }

    validate_destination_root(dest_root)?;
    if sources
        .iter()
        .any(|source| paths_overlap(dest_root, source))
    {
        return Err(SqobaError::InvalidArchive(
            "SQOBA destination overlaps a save root".into(),
        ));
    }
    let game_dir = dest_root.join(game_id);
    std::fs::create_dir_all(&game_dir)?;
    validate_destination_root(&game_dir)?;
    let _lock = acquire_game_lock(&game_dir)?;
    recover_pending_journals(&game_dir, Some(game_id))?;
    let mut backup = create_verified_backup(game_id, &sources, &game_dir)?;
    if let Err(error) = rotate_backups(&game_dir, keep, &backup.dest_path) {
        backup
            .warnings
            .push(format!("backup rotation failed: {error}"));
    }
    Ok(backup)
}

fn unique_backup_path(game_dir: &Path, timestamp: &str) -> PathBuf {
    let first = game_dir.join(format!("{timestamp}.zip"));
    if !first.exists() {
        return first;
    }

    for index in 1..1000 {
        let candidate = game_dir.join(format!("{timestamp}-{index}.zip"));
        if !candidate.exists() {
            return candidate;
        }
    }

    game_dir.join(format!("{timestamp}-{}.zip", uuid::Uuid::new_v4()))
}

fn create_verified_backup(
    game_id: &str,
    sources: &[PathBuf],
    output_dir: &Path,
) -> Result<SqobaBackup, SqobaError> {
    create_verified_backup_with_options(game_id, sources, output_dir, false)
}

fn create_verified_snapshot(
    game_id: &str,
    sources: &[PathBuf],
    output_dir: &Path,
) -> Result<SqobaBackup, SqobaError> {
    create_verified_backup_with_options(game_id, sources, output_dir, true)
}

fn create_verified_backup_with_options(
    game_id: &str,
    sources: &[PathBuf],
    output_dir: &Path,
    allow_missing_roots: bool,
) -> Result<SqobaBackup, SqobaError> {
    let sources = if allow_missing_roots {
        validate_archive_source_roots(sources)?
    } else {
        validate_source_roots(sources)?
    };
    let id = uuid::Uuid::new_v4().to_string();
    let file_timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let dest_path = unique_backup_path(output_dir, &file_timestamp);
    let temp_path = dest_path.with_file_name(format!(".{}.{}.tmp", file_timestamp, id));
    let result = (|| {
        let (files_count, bytes) = write_archive(
            &temp_path,
            game_id,
            &id,
            &timestamp,
            &sources,
            allow_missing_roots,
        )?;
        verify_archive(&temp_path, Some(game_id))?;
        atomic_replace(&temp_path, &dest_path)?;
        sync_directory(output_dir)?;
        Ok(SqobaBackup {
            id,
            game_id: game_id.to_string(),
            timestamp,
            dest_path,
            files_count,
            bytes,
            source_paths: sources,
            warnings: Vec::new(),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

fn write_archive(
    path: &Path,
    game_id: &str,
    id: &str,
    timestamp: &str,
    sources: &[PathBuf],
    allow_missing_roots: bool,
) -> Result<(u32, u64), SqobaError> {
    let file = std::fs::File::create(path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files_count = 0u32;
    let mut bytes = 0u64;
    let mut archive_entries = 0usize;
    for (idx, src) in sources.iter().enumerate() {
        let prefix = format!("source_{idx}");
        archive_entries += 1;
        zw.add_directory(&prefix, opts)?;
        if !src.exists() {
            if allow_missing_roots {
                continue;
            }
            return Err(SqobaError::InvalidArchive(format!(
                "source root disappeared: {}",
                src.display()
            )));
        }
        let metadata = std::fs::symlink_metadata(src)?;
        if metadata_is_link(&metadata) || !metadata.is_dir() {
            return Err(SqobaError::InvalidArchive(format!(
                "source root is not a real directory: {}",
                src.display()
            )));
        }
        write_dir_recursive(
            &mut zw,
            src,
            src,
            &prefix,
            opts,
            &mut files_count,
            &mut bytes,
            &mut archive_entries,
        )?;
    }
    if archive_entries >= MAX_ARCHIVE_ENTRIES {
        return Err(SqobaError::InvalidArchive(
            "too many archive entries".into(),
        ));
    }
    let meta = ZipMeta {
        format_version: SQOBA_FORMAT_VERSION,
        id: id.to_string(),
        game_id: game_id.to_string(),
        timestamp: timestamp.to_string(),
        source_root_count: sources.len() as u32,
        files_count,
        bytes,
        source_paths: sources.to_vec(),
    };
    zw.start_file(META_FILENAME, opts)?;
    zw.write_all(&serde_json::to_vec_pretty(&meta)?)?;
    let file = zw.finish()?;
    file.sync_all()?;
    Ok((files_count, bytes))
}

fn write_dir_recursive<W: Write + std::io::Seek>(
    zw: &mut zip::ZipWriter<W>,
    root: &Path,
    cur: &Path,
    prefix: &str,
    opts: zip::write::FileOptions,
    files_count: &mut u32,
    bytes: &mut u64,
    archive_entries: &mut usize,
) -> Result<(), SqobaError> {
    for entry in std::fs::read_dir(cur)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let zip_name = format!("{}/{}", prefix, rel_str);
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata_is_link(&metadata) {
            return Err(SqobaError::InvalidArchive(format!(
                "link or reparse point in save root: {}",
                path.display()
            )));
        }
        let ftype = metadata.file_type();
        if ftype.is_dir() {
            if *archive_entries >= MAX_ARCHIVE_ENTRIES - 1 {
                return Err(SqobaError::InvalidArchive(
                    "too many archive entries".into(),
                ));
            }
            *archive_entries += 1;
            zw.add_directory(&zip_name, opts)?;
            write_dir_recursive(
                zw,
                root,
                &path,
                prefix,
                opts,
                files_count,
                bytes,
                archive_entries,
            )?;
        } else if ftype.is_file() {
            if *archive_entries >= MAX_ARCHIVE_ENTRIES - 1 {
                return Err(SqobaError::InvalidArchive(
                    "too many archive entries".into(),
                ));
            }
            let declared_size = metadata.len();
            if declared_size > MAX_ENTRY_BYTES
                || bytes.saturating_add(declared_size) > MAX_TOTAL_BYTES
            {
                return Err(SqobaError::InvalidArchive(
                    "save data exceeds SQOBA limits".into(),
                ));
            }
            *archive_entries += 1;
            zw.start_file(&zip_name, opts)?;
            let mut f = std::fs::File::open(&path)?;
            let copied = io::copy(&mut f, zw)?;
            if copied > MAX_ENTRY_BYTES || bytes.saturating_add(copied) > MAX_TOTAL_BYTES {
                return Err(SqobaError::InvalidArchive(
                    "save data exceeds SQOBA limits".into(),
                ));
            }
            *files_count = files_count
                .checked_add(1)
                .ok_or_else(|| SqobaError::InvalidArchive("too many files".into()))?;
            *bytes += copied;
        } else {
            return Err(SqobaError::InvalidArchive(format!(
                "unsupported save entry: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn rotate_backups(game_dir: &Path, keep: u32, protected: &Path) -> Result<(), SqobaError> {
    if keep == 0 {
        return Ok(());
    }
    let mut zips: Vec<PathBuf> = Vec::new();
    if !game_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(game_dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("zip") {
            zips.push(p);
        }
    }
    // Sort by filename (timestamp prefix) descending — самый свежий первый.
    zips.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    for old in zips.into_iter().skip(keep as usize) {
        if old == protected {
            continue;
        }
        std::fs::remove_file(old)?;
    }
    Ok(())
}

// ───────────────────────── list ─────────────────────────

pub fn list_backups(game_id: &str) -> Vec<SqobaBackup> {
    list_backups_with_root(game_id, &dest_root())
}

pub fn list_backups_with_root(game_id: &str, dest_root: &Path) -> Vec<SqobaBackup> {
    if validate_game_id(game_id).is_err() {
        return Vec::new();
    }
    if validate_destination_root(dest_root).is_err() {
        return Vec::new();
    }
    let game_dir = dest_root.join(game_id);
    if validate_destination_root(&game_dir).is_err() || !game_dir.exists() {
        return Vec::new();
    }
    let mut out: Vec<SqobaBackup> = Vec::new();
    let entries = match std::fs::read_dir(&game_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("zip") {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata_is_link(&metadata) || !metadata.is_file() {
            continue;
        }
        if let Some(b) = inspect_backup(game_id, &path) {
            out.push(b);
        }
    }
    out.sort_by(|a, b| b.dest_path.file_name().cmp(&a.dest_path.file_name()));
    out
}

fn inspect_backup(game_id: &str, path: &Path) -> Option<SqobaBackup> {
    let plan = verify_archive(path, Some(game_id)).ok()?;

    Some(SqobaBackup {
        id: plan.meta.id,
        game_id: game_id.to_string(),
        timestamp: plan.meta.timestamp,
        dest_path: path.to_path_buf(),
        files_count: plan.meta.files_count,
        bytes: plan.meta.bytes,
        source_paths: plan.meta.source_paths,
        warnings: Vec::new(),
    })
}

// ───────────────────────── restore ─────────────────────────

pub fn restore(backup_path: &Path) -> Result<RestoreResult, SqobaError> {
    restore_for_game(backup_path, None)
}

pub fn restore_for_game(
    backup_path: &Path,
    expected_game_id: Option<&str>,
) -> Result<RestoreResult, SqobaError> {
    let game_dir = backup_path
        .parent()
        .ok_or_else(|| SqobaError::BackupNotFound(backup_path.display().to_string()))?;
    if let Some(game_id) = expected_game_id {
        validate_game_id(game_id)?;
        if !same_game_id(game_dir.file_name().and_then(|s| s.to_str()), game_id) {
            return Ok(rolled_back(vec!["backup is outside selected game".into()]));
        }
    }
    validate_destination_root(game_dir)?;
    std::fs::create_dir_all(game_dir)?;
    validate_destination_root(game_dir)?;
    let _lock = acquire_game_lock(game_dir)?;

    if let Ok(metadata) = std::fs::symlink_metadata(backup_path) {
        if metadata_is_link(&metadata) || !metadata.is_file() {
            return Ok(rolled_back(vec!["backup is not a regular file".into()]));
        }
    }

    if let Err(error) = recover_pending_journals(game_dir, expected_game_id) {
        return Ok(RestoreResult {
            status: RestoreStatus::ManualRecoveryRequired,
            restored_files: 0,
            bytes: 0,
            errors: vec![format!("pending restore recovery failed: {error}")],
            recovery_backup: None,
        });
    }

    let plan = match verify_archive(backup_path, expected_game_id) {
        Ok(plan) => plan,
        Err(error) => return Ok(rolled_back(vec![error.to_string()])),
    };
    if expected_game_id.is_none()
        && !same_game_id(
            game_dir.file_name().and_then(|name| name.to_str()),
            &plan.meta.game_id,
        )
    {
        return Ok(rolled_back(vec![
            "backup is outside its game directory".into()
        ]));
    }
    if plan
        .meta
        .source_paths
        .iter()
        .any(|source| paths_overlap(game_dir, source))
    {
        return Ok(rolled_back(vec![
            "SQOBA destination overlaps a save root".into()
        ]));
    }
    let game_id = expected_game_id.unwrap_or(&plan.meta.game_id);
    let recovery_dir = game_dir.join(RECOVERY_DIR);
    std::fs::create_dir_all(&recovery_dir)?;
    let snapshot = match create_verified_snapshot(game_id, &plan.meta.source_paths, &recovery_dir) {
        Ok(snapshot) => snapshot,
        Err(error) => return Ok(rolled_back(vec![format!("safety backup failed: {error}")])),
    };

    let staging_dir = recovery_dir.join(format!(".staging-{}", uuid::Uuid::new_v4()));
    if let Err(error) = extract_to_staging(backup_path, &plan, &staging_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Ok(RestoreResult {
            status: RestoreStatus::RolledBack,
            restored_files: 0,
            bytes: 0,
            errors: vec![format!("staging failed: {error}")],
            recovery_backup: Some(snapshot.dest_path),
        });
    }

    let entries = plan
        .entries
        .iter()
        .map(|entry| JournalEntry {
            target: entry.target.clone(),
            snapshot_member: entry.member.clone(),
            was_present: std::fs::symlink_metadata(&entry.target)
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false),
        })
        .collect();
    let journal = RestoreJournal {
        version: SQOBA_FORMAT_VERSION,
        game_id: game_id.to_string(),
        backup_id: plan.meta.id.clone(),
        backup_path: backup_path.to_path_buf(),
        snapshot_path: snapshot.dest_path.clone(),
        staging_dir: staging_dir.clone(),
        entries,
    };
    let journal_path = recovery_dir.join(format!("{}{}", journal.backup_id, JOURNAL_SUFFIX));
    if let Err(error) = write_json_atomic(&journal_path, &journal) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Ok(RestoreResult {
            status: RestoreStatus::RolledBack,
            restored_files: 0,
            bytes: 0,
            errors: vec![format!("journal creation failed: {error}")],
            recovery_backup: Some(snapshot.dest_path),
        });
    }

    let commit_result = (|| {
        ensure_restore_roots(&plan.meta.source_paths)
            .map_err(|error| format!("restore root creation failed: {error}"))?;
        plan.entries.iter().enumerate().try_fold(
            (0u32, 0u64),
            |(restored_files, bytes), (index, entry)| {
                let staged = staging_dir.join(&entry.member);
                install_staged(&staged, &entry.target)
                    .map(|()| (restored_files + 1, bytes.saturating_add(entry.size)))
                    .map_err(|error| format!("commit entry {index} ({}): {error}", entry.member))
            },
        )
    })();

    match commit_result {
        Ok((restored_files, bytes)) => {
            match cleanup_recovery_artifacts(&journal_path, &staging_dir, &recovery_dir) {
                Ok(()) => Ok(RestoreResult {
                    status: RestoreStatus::Committed,
                    restored_files,
                    bytes,
                    errors: Vec::new(),
                    recovery_backup: Some(snapshot.dest_path),
                }),
                Err(error) => Ok(RestoreResult {
                    status: RestoreStatus::ManualRecoveryRequired,
                    restored_files,
                    bytes,
                    errors: vec![format!("restore committed but cleanup failed: {error}")],
                    recovery_backup: Some(snapshot.dest_path),
                }),
            }
        }
        Err(error) => match rollback_journal(&journal, game_dir) {
            Ok(()) => {
                match cleanup_recovery_artifacts(&journal_path, &staging_dir, &recovery_dir) {
                    Ok(()) => Ok(RestoreResult {
                        status: RestoreStatus::RolledBack,
                        restored_files: 0,
                        bytes: 0,
                        errors: vec![error],
                        recovery_backup: Some(snapshot.dest_path),
                    }),
                    Err(cleanup_error) => Ok(RestoreResult {
                        status: RestoreStatus::ManualRecoveryRequired,
                        restored_files: 0,
                        bytes: 0,
                        errors: vec![error, format!("rollback cleanup failed: {cleanup_error}")],
                        recovery_backup: Some(snapshot.dest_path),
                    }),
                }
            }
            Err(rollback_error) => Ok(RestoreResult {
                status: RestoreStatus::ManualRecoveryRequired,
                restored_files: 0,
                bytes: 0,
                errors: vec![error, format!("rollback failed: {rollback_error}")],
                recovery_backup: Some(snapshot.dest_path),
            }),
        },
    }
}

fn rolled_back(errors: Vec<String>) -> RestoreResult {
    RestoreResult {
        status: RestoreStatus::RolledBack,
        restored_files: 0,
        bytes: 0,
        errors,
        recovery_backup: None,
    }
}

fn validate_game_id(game_id: &str) -> Result<(), SqobaError> {
    if game_id.is_empty()
        || game_id == "."
        || game_id == ".."
        || game_id.ends_with('.')
        || game_id.ends_with(' ')
        || game_id
            .chars()
            .any(|character| matches!(character, '/' | '\\' | ':'))
        || !is_safe_windows_component(game_id)
    {
        return Err(SqobaError::InvalidArchive("invalid game id".into()));
    }
    Ok(())
}

fn is_safe_windows_component(component: &str) -> bool {
    #[cfg(windows)]
    {
        let stem = component
            .trim_end_matches(['.', ' '])
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$") {
            return false;
        }
        if let Some(number) = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
        {
            return number.len() == 1
                && number
                    .chars()
                    .all(|character| ('1'..='9').contains(&character));
        }
    }
    true
}

fn validate_opaque_id(id: &str) -> Result<(), SqobaError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(SqobaError::InvalidArchive(
            "invalid archive identity".into(),
        ));
    }
    Ok(())
}

fn acquire_game_lock(game_dir: &Path) -> Result<GameLock, SqobaError> {
    let path = game_dir.join(LOCK_FILENAME);
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id())?;
                file.sync_all()?;
                return Ok(GameLock { path, _file: file });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|value| value.trim().parse::<u32>().ok());
                if pid.is_some_and(|pid| !process_is_alive(pid)) {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                return Err(SqobaError::Busy);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(SqobaError::Busy)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let mut exit_code = 0;
    let alive = unsafe { GetExitCodeProcess(handle, &mut exit_code).is_ok() } && exit_code == 259;
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
    alive
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

#[cfg(not(any(unix, windows)))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

fn metadata_is_link(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn validate_source_roots(paths: &[PathBuf]) -> Result<Vec<PathBuf>, SqobaError> {
    validate_source_roots_with_options(paths, true)
}

fn validate_archive_source_roots(paths: &[PathBuf]) -> Result<Vec<PathBuf>, SqobaError> {
    validate_source_roots_with_options(paths, false)
}

fn validate_source_roots_with_options(
    paths: &[PathBuf],
    require_existing: bool,
) -> Result<Vec<PathBuf>, SqobaError> {
    if paths.is_empty() || paths.len() > MAX_SOURCE_ROOTS {
        return Err(SqobaError::InvalidArchive(
            "invalid source root count".into(),
        ));
    }
    let mut seen = HashSet::new();
    let mut canonical: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        if !require_existing
            && (!path.is_absolute()
                || path
                    .components()
                    .any(|component| matches!(component, Component::CurDir | Component::ParentDir)))
        {
            return Err(SqobaError::InvalidArchive(format!(
                "source root is not a safe absolute path: {}",
                path.display()
            )));
        }
        let metadata = std::fs::symlink_metadata(path);
        if require_existing {
            let metadata = metadata.map_err(|error| {
                SqobaError::InvalidArchive(format!("source root {}: {error}", path.display()))
            })?;
            if !metadata.is_dir() || metadata_is_link(&metadata) {
                return Err(SqobaError::InvalidArchive(format!(
                    "source root is not a real directory: {}",
                    path.display()
                )));
            }
        } else if let Ok(metadata) = metadata {
            if !metadata.is_dir() || metadata_is_link(&metadata) {
                return Err(SqobaError::InvalidArchive(format!(
                    "source root is not a real directory: {}",
                    path.display()
                )));
            }
        }
        ensure_no_link_ancestors(path)?;
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            path.to_path_buf()
        };
        let key = archive_target_key(&path);
        if !seen.insert(key) {
            return Err(SqobaError::InvalidArchive("duplicate source root".into()));
        }
        if canonical
            .iter()
            .any(|existing| paths_overlap(existing, &path))
        {
            return Err(SqobaError::InvalidArchive(
                "overlapping source roots".into(),
            ));
        }
        canonical.push(path);
    }
    Ok(canonical)
}

fn normalize_archive_path(raw: &str, is_dir: bool) -> Result<String, SqobaError> {
    if raw.is_empty() || raw.contains('\0') {
        return Err(SqobaError::InvalidArchive(
            "empty or NUL archive path".into(),
        ));
    }
    let raw = raw.replace('\\', "/");
    if raw.starts_with('/')
        || raw.starts_with("//")
        || raw.starts_with('\\')
        || raw.as_bytes().get(1) == Some(&b':')
    {
        return Err(SqobaError::InvalidArchive(format!(
            "absolute archive path: {raw}"
        )));
    }
    let raw = if is_dir {
        raw.strip_suffix('/').unwrap_or(&raw)
    } else {
        raw.as_str()
    };
    let mut components = Vec::new();
    for component in raw.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(':')
            || component.ends_with('.')
            || component.ends_with(' ')
            || !is_safe_windows_component(component)
        {
            return Err(SqobaError::InvalidArchive(format!(
                "unsafe archive path: {raw}"
            )));
        }
        components.push(component);
    }
    if components.is_empty() {
        return Err(SqobaError::InvalidArchive("empty archive path".into()));
    }
    Ok(components.join("/"))
}

fn relative_path(member: &str) -> PathBuf {
    member
        .split('/')
        .fold(PathBuf::new(), |mut path, component| {
            path.push(component);
            path
        })
}

fn ensure_safe_target(root: &Path, relative: &Path) -> Result<PathBuf, SqobaError> {
    if !root.is_absolute() {
        return Err(SqobaError::InvalidArchive(format!(
            "source root is not absolute: {}",
            root.display()
        )));
    }
    let target = root.join(relative);
    if !target.starts_with(root) {
        return Err(SqobaError::InvalidArchive(format!(
            "target escapes source root: {}",
            target.display()
        )));
    }
    let mut current = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        if let Ok(metadata) = std::fs::symlink_metadata(&current) {
            if metadata_is_link(&metadata) {
                return Err(SqobaError::InvalidArchive(format!(
                    "target crosses a link or reparse point: {}",
                    current.display()
                )));
            }
            if index + 1 < components.len() && !metadata.is_dir() {
                return Err(SqobaError::InvalidArchive(format!(
                    "target parent is not a directory: {}",
                    current.display()
                )));
            }
        }
    }
    Ok(target)
}

fn ensure_no_link_ancestors(path: &Path) -> Result<(), SqobaError> {
    for ancestor in path.ancestors() {
        if let Ok(metadata) = std::fs::symlink_metadata(ancestor) {
            if metadata_is_link(&metadata) {
                return Err(SqobaError::InvalidArchive(format!(
                    "path crosses a link or reparse point: {}",
                    ancestor.display()
                )));
            }
        }
    }
    Ok(())
}

fn validate_destination_root(path: &Path) -> Result<(), SqobaError> {
    ensure_no_link_ancestors(path)?;
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata_is_link(&metadata) || !metadata.is_dir() {
            return Err(SqobaError::InvalidArchive(format!(
                "SQOBA destination is not a real directory: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn comparable_path(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let path = path.canonicalize().unwrap_or(path);
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        PathBuf::from(
            value
                .strip_prefix("\\\\?\\")
                .unwrap_or(&value)
                .to_ascii_lowercase(),
        )
    }
    #[cfg(not(windows))]
    {
        path
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    let left = comparable_path(left);
    let right = comparable_path(right);
    left.starts_with(&right) || right.starts_with(&left)
}

fn same_game_id(left: Option<&str>, right: &str) -> bool {
    #[cfg(windows)]
    {
        left.is_some_and(|left| left.eq_ignore_ascii_case(right))
    }
    #[cfg(not(windows))]
    {
        left == Some(right)
    }
}

fn archive_target_key(path: &Path) -> String {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        value
            .strip_prefix("\\\\?\\")
            .unwrap_or(&value)
            .to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().into_owned()
    }
}

fn parse_archive_plan(
    archive: &mut zip::ZipArchive<std::fs::File>,
    expected_game_id: Option<&str>,
) -> Result<ArchivePlan, SqobaError> {
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(SqobaError::InvalidArchive(
            "too many archive entries".into(),
        ));
    }
    let mut meta_index = None;
    let mut meta_count = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.name() == META_FILENAME {
            meta_count += 1;
            if entry.is_dir() || entry.size() > MAX_META_BYTES {
                return Err(SqobaError::InvalidArchive("invalid metadata entry".into()));
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(MAX_META_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_META_BYTES {
                return Err(SqobaError::InvalidArchive("metadata is too large".into()));
            }
            meta_index = Some((index, bytes));
        }
    }
    if meta_count != 1 {
        return Err(SqobaError::InvalidArchive(
            "archive must contain exactly one metadata entry".into(),
        ));
    }
    let (_, meta_bytes) = meta_index.expect("meta_count checked");
    let mut meta: ZipMeta = serde_json::from_slice(&meta_bytes)?;
    if meta.format_version != SQOBA_FORMAT_VERSION {
        return Err(SqobaError::InvalidArchive(format!(
            "unsupported SQOBA format version {}",
            meta.format_version
        )));
    }
    validate_opaque_id(&meta.id)?;
    validate_game_id(&meta.game_id)?;
    if expected_game_id.is_some_and(|game_id| game_id != meta.game_id) {
        return Err(SqobaError::InvalidArchive(
            "backup belongs to another game".into(),
        ));
    }
    if meta.source_root_count as usize != meta.source_paths.len()
        || meta.source_paths.len() > MAX_SOURCE_ROOTS
    {
        return Err(SqobaError::InvalidArchive(
            "metadata source root count mismatch".into(),
        ));
    }
    meta.source_paths = validate_archive_source_roots(&meta.source_paths)?;

    let mut names = HashSet::new();
    let mut targets = HashSet::new();
    let mut roots = vec![false; meta.source_paths.len()];
    let mut entries = Vec::new();
    let mut files_count = 0u32;
    let mut bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.name() == META_FILENAME {
            continue;
        }
        let normalized = normalize_archive_path(entry.name(), entry.is_dir())?;
        if !names.insert(normalized.to_lowercase()) {
            return Err(SqobaError::InvalidArchive(format!(
                "duplicate archive path: {}",
                entry.name()
            )));
        }
        let (root_name, relative_name) = match normalized.split_once('/') {
            Some((root_name, relative_name)) => (root_name, relative_name),
            None if entry.is_dir() => (normalized.as_str(), ""),
            None => {
                return Err(SqobaError::InvalidArchive(format!(
                    "malformed archive path: {}",
                    entry.name()
                )))
            }
        };
        let root_index = root_name
            .strip_prefix("source_")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| format!("source_{index}") == root_name)
            .ok_or_else(|| {
                SqobaError::InvalidArchive(format!("unknown archive root: {root_name}"))
            })?;
        let root = meta
            .source_paths
            .get(root_index)
            .ok_or_else(|| SqobaError::InvalidArchive("archive root index out of range".into()))?;
        roots[root_index] = true;
        let file_type = entry.unix_mode().map(|mode| mode & 0o170000);
        if file_type == Some(0o120000) {
            return Err(SqobaError::InvalidArchive("symlink entry rejected".into()));
        }
        if entry.is_dir() {
            if file_type.is_some_and(|kind| kind != 0 && kind != 0o040000) {
                return Err(SqobaError::InvalidArchive(
                    "unsupported directory type".into(),
                ));
            }
            if !relative_name.is_empty() {
                let _ = ensure_safe_target(root, &relative_path(relative_name))?;
            }
            continue;
        }
        if file_type.is_some_and(|kind| kind != 0 && kind != 0o100000) {
            return Err(SqobaError::InvalidArchive("unsupported file type".into()));
        }
        if relative_name.is_empty() {
            return Err(SqobaError::InvalidArchive(
                "source root cannot be a file".into(),
            ));
        }
        if entry.size() > MAX_ENTRY_BYTES || bytes.saturating_add(entry.size()) > MAX_TOTAL_BYTES {
            return Err(SqobaError::InvalidArchive(
                "archive expands beyond limits".into(),
            ));
        }
        let relative = relative_path(relative_name);
        let target = ensure_safe_target(root, &relative)?;
        if !targets.insert(archive_target_key(&target)) {
            return Err(SqobaError::InvalidArchive(format!(
                "duplicate target path: {}",
                target.display()
            )));
        }
        if std::fs::symlink_metadata(&target)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
        {
            return Err(SqobaError::InvalidArchive(format!(
                "file target is a directory: {}",
                target.display()
            )));
        }
        files_count = files_count
            .checked_add(1)
            .ok_or_else(|| SqobaError::InvalidArchive("too many files".into()))?;
        bytes += entry.size();
        entries.push(ArchiveEntry {
            index,
            member: normalized,
            target,
            size: entry.size(),
        });
    }
    if roots.iter().any(|seen| !seen) || files_count != meta.files_count || bytes != meta.bytes {
        return Err(SqobaError::InvalidArchive(
            "archive contents do not match metadata".into(),
        ));
    }
    Ok(ArchivePlan { meta, entries })
}

fn verify_archive(path: &Path, expected_game_id: Option<&str>) -> Result<ArchivePlan, SqobaError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let plan = parse_archive_plan(&mut archive, expected_game_id)?;
    for entry in &plan.entries {
        let mut zip_entry = archive.by_index(entry.index)?;
        let copied = io::copy(
            &mut (&mut zip_entry).take(entry.size.saturating_add(1)),
            &mut io::sink(),
        )?;
        if copied != entry.size {
            return Err(SqobaError::InvalidArchive(format!(
                "entry size mismatch: {}",
                entry.member
            )));
        }
    }
    Ok(plan)
}

fn extract_to_staging(
    backup_path: &Path,
    plan: &ArchivePlan,
    staging_dir: &Path,
) -> Result<(), SqobaError> {
    std::fs::create_dir_all(staging_dir)?;
    let file = std::fs::File::open(backup_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for entry in &plan.entries {
        let output = staging_dir.join(relative_path(&entry.member));
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut zip_entry = archive.by_index(entry.index)?;
        let mut output_file = std::fs::File::create(&output)?;
        let copied = io::copy(
            &mut (&mut zip_entry).take(entry.size.saturating_add(1)),
            &mut output_file,
        )?;
        if copied != entry.size {
            return Err(SqobaError::InvalidArchive(format!(
                "entry size mismatch: {}",
                entry.member
            )));
        }
        output_file.sync_all()?;
    }
    Ok(())
}

fn install_staged(staged: &Path, target: &Path) -> Result<(), SqobaError> {
    let parent = target
        .parent()
        .ok_or_else(|| SqobaError::InvalidArchive("target has no parent".into()))?;
    ensure_no_link_ancestors(parent)?;
    std::fs::create_dir_all(parent)?;
    ensure_no_link_ancestors(parent)?;
    if let Ok(metadata) = std::fs::symlink_metadata(target) {
        if metadata_is_link(&metadata) || !metadata.is_file() {
            return Err(SqobaError::InvalidArchive(format!(
                "unsafe target: {}",
                target.display()
            )));
        }
    }
    if atomic_replace(staged, target).is_ok() {
        return Ok(());
    }
    let temporary = parent.join(format!(".sqoba-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut input = std::fs::File::open(staged)?;
        let mut output = std::fs::File::create(&temporary)?;
        io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        atomic_replace(&temporary, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

fn ensure_restore_roots(roots: &[PathBuf]) -> Result<(), SqobaError> {
    for root in roots {
        if !root.is_absolute() {
            return Err(SqobaError::InvalidArchive(format!(
                "source root is not absolute: {}",
                root.display()
            )));
        }
        ensure_no_link_ancestors(root)?;
        if let Ok(metadata) = std::fs::symlink_metadata(root) {
            if metadata_is_link(&metadata) || !metadata.is_dir() {
                return Err(SqobaError::InvalidArchive(format!(
                    "source root is not a directory: {}",
                    root.display()
                )));
            }
        }
        std::fs::create_dir_all(root)?;
        ensure_no_link_ancestors(root)?;
    }
    Ok(())
}

fn validate_journal(journal: &RestoreJournal, game_dir: &Path) -> Result<(), SqobaError> {
    validate_game_id(&journal.game_id)?;
    validate_opaque_id(&journal.backup_id)?;
    if !same_game_id(
        game_dir.file_name().and_then(|name| name.to_str()),
        &journal.game_id,
    ) {
        return Err(SqobaError::InvalidArchive(
            "recovery journal belongs to another game".into(),
        ));
    }
    let recovery_dir = game_dir.join(RECOVERY_DIR);
    if !journal
        .backup_path
        .parent()
        .is_some_and(|parent| same_path(parent, game_dir))
        || journal.backup_path.extension().and_then(|ext| ext.to_str()) != Some("zip")
        || !journal
            .snapshot_path
            .parent()
            .is_some_and(|parent| same_path(parent, &recovery_dir))
        || journal
            .snapshot_path
            .extension()
            .and_then(|ext| ext.to_str())
            != Some("zip")
    {
        return Err(SqobaError::InvalidArchive(
            "recovery journal path is outside its game directory".into(),
        ));
    }
    let staging_name = journal
        .staging_dir
        .parent()
        .filter(|parent| same_path(parent, &recovery_dir))
        .and_then(|_| journal.staging_dir.file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(".staging-"))
        .ok_or_else(|| SqobaError::InvalidArchive("invalid recovery staging path".into()))?;
    validate_opaque_id(staging_name)?;
    ensure_no_link_ancestors(game_dir)?;
    ensure_no_link_ancestors(&recovery_dir)?;
    if let Ok(metadata) = std::fs::symlink_metadata(&journal.staging_dir) {
        if metadata_is_link(&metadata) || !metadata.is_dir() {
            return Err(SqobaError::InvalidArchive(
                "recovery staging path is not a directory".into(),
            ));
        }
    }

    for path in [&journal.backup_path, &journal.snapshot_path] {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata_is_link(&metadata) || !metadata.is_file() {
            return Err(SqobaError::InvalidArchive(format!(
                "recovery archive is not a regular file: {}",
                path.display()
            )));
        }
    }
    let backup_plan = verify_archive(&journal.backup_path, Some(&journal.game_id))?;
    if backup_plan.meta.id != journal.backup_id {
        return Err(SqobaError::InvalidArchive(
            "recovery journal backup identity mismatch".into(),
        ));
    }
    let snapshot_plan = verify_archive(&journal.snapshot_path, Some(&journal.game_id))?;
    let backup_targets: HashMap<&str, &Path> = backup_plan
        .entries
        .iter()
        .map(|entry| (entry.member.as_str(), entry.target.as_path()))
        .collect();
    let snapshot_members: HashSet<&str> = snapshot_plan
        .entries
        .iter()
        .map(|entry| entry.member.as_str())
        .collect();
    let mut journal_members = HashSet::new();
    for entry in &journal.entries {
        let target = backup_targets
            .get(entry.snapshot_member.as_str())
            .ok_or_else(|| {
                SqobaError::InvalidArchive(format!(
                    "recovery journal entry is not in backup: {}",
                    entry.snapshot_member
                ))
            })?;
        if *target != entry.target.as_path() {
            return Err(SqobaError::InvalidArchive(format!(
                "recovery journal target mismatch: expected {}, got {}",
                target.display(),
                entry.target.display()
            )));
        }
        if !journal_members.insert(entry.snapshot_member.as_str()) {
            return Err(SqobaError::InvalidArchive(
                "recovery journal entry mismatch".into(),
            ));
        }
        if snapshot_members.contains(entry.snapshot_member.as_str()) != entry.was_present {
            return Err(SqobaError::InvalidArchive(
                "recovery journal snapshot presence mismatch".into(),
            ));
        }
    }
    if journal_members.len() != backup_targets.len() {
        return Err(SqobaError::InvalidArchive(
            "recovery journal is incomplete".into(),
        ));
    }
    Ok(())
}

fn cleanup_recovery_artifacts(
    journal_path: &Path,
    staging_dir: &Path,
    recovery_dir: &Path,
) -> Result<(), SqobaError> {
    if let Ok(metadata) = std::fs::symlink_metadata(staging_dir) {
        if metadata_is_link(&metadata) || !metadata.is_dir() {
            return Err(SqobaError::InvalidArchive(
                "recovery staging path is not a directory".into(),
            ));
        }
        std::fs::remove_dir_all(staging_dir)?;
    }
    if std::fs::symlink_metadata(journal_path).is_ok() {
        std::fs::remove_file(journal_path)?;
    }
    sync_directory(recovery_dir)?;
    Ok(())
}

fn rollback_journal(journal: &RestoreJournal, game_dir: &Path) -> Result<(), SqobaError> {
    validate_journal(journal, game_dir)?;
    let file = std::fs::File::open(&journal.snapshot_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for entry in &journal.entries {
        if entry.was_present {
            let mut snapshot = archive.by_name(&entry.snapshot_member).map_err(|_| {
                SqobaError::InvalidArchive(format!(
                    "snapshot entry missing: {}",
                    entry.snapshot_member
                ))
            })?;
            let parent = entry.target.parent().ok_or_else(|| {
                SqobaError::InvalidArchive("snapshot target has no parent".into())
            })?;
            ensure_no_link_ancestors(parent)?;
            if let Ok(metadata) = std::fs::symlink_metadata(&entry.target) {
                if metadata_is_link(&metadata) || !metadata.is_file() {
                    return Err(SqobaError::InvalidArchive(format!(
                        "unsafe rollback target: {}",
                        entry.target.display()
                    )));
                }
            }
            std::fs::create_dir_all(parent)?;
            let temporary = parent.join(format!(".sqoba-rollback-{}.tmp", uuid::Uuid::new_v4()));
            let result = (|| {
                let mut output = std::fs::File::create(&temporary)?;
                let size = snapshot.size();
                let copied = io::copy(
                    &mut (&mut snapshot).take(size.saturating_add(1)),
                    &mut output,
                )?;
                if copied != size {
                    return Err(std::io::Error::other("snapshot entry size mismatch"));
                }
                output.sync_all()?;
                atomic_replace(&temporary, &entry.target)
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temporary);
            }
            result?;
        } else if let Ok(metadata) = std::fs::symlink_metadata(&entry.target) {
            if metadata_is_link(&metadata) || !metadata.is_file() {
                return Err(SqobaError::InvalidArchive(format!(
                    "cannot remove unsafe rollback target: {}",
                    entry.target.display()
                )));
            }
            std::fs::remove_file(&entry.target)?;
        }
    }
    Ok(())
}

fn recover_pending_journals(
    game_dir: &Path,
    expected_game_id: Option<&str>,
) -> Result<(), SqobaError> {
    let recovery_dir = game_dir.join(RECOVERY_DIR);
    if !recovery_dir.exists() {
        return Ok(());
    }
    let recovery_metadata = std::fs::symlink_metadata(&recovery_dir)?;
    if metadata_is_link(&recovery_metadata) || !recovery_metadata.is_dir() {
        return Err(SqobaError::InvalidArchive(
            "invalid recovery directory".into(),
        ));
    }
    for entry in std::fs::read_dir(&recovery_dir)? {
        let path = entry?.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(JOURNAL_SUFFIX))
        {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata_is_link(&metadata) || !metadata.is_file() {
            return Err(SqobaError::InvalidArchive(
                "invalid recovery journal file".into(),
            ));
        }
        let journal: RestoreJournal = serde_json::from_slice(&std::fs::read(&path)?)?;
        if journal.version != SQOBA_FORMAT_VERSION
            || expected_game_id.is_some_and(|game_id| game_id != journal.game_id)
        {
            return Err(SqobaError::InvalidArchive(
                "invalid recovery journal".into(),
            ));
        }
        let expected_name = format!("{}{}", journal.backup_id, JOURNAL_SUFFIX);
        if !same_path(&path, &recovery_dir.join(expected_name)) {
            return Err(SqobaError::InvalidArchive(
                "recovery journal filename mismatch".into(),
            ));
        }
        rollback_journal(&journal, game_dir)?;
        cleanup_recovery_artifacts(&path, &journal.staging_dir, &recovery_dir)?;
    }
    Ok(())
}

pub fn recover_pending_at_startup() {
    let root = dest_root();
    let Ok(root_metadata) = std::fs::symlink_metadata(&root) else {
        return;
    };
    if metadata_is_link(&root_metadata) || !root_metadata.is_dir() {
        tracing::error!(path = ?root, "SQOBA recovery root is not a directory");
        return;
    }
    let Ok(entries) = std::fs::read_dir(&root) else {
        tracing::error!(path = ?root, "failed to scan SQOBA recovery root");
        return;
    };
    for entry in entries.flatten() {
        let game_dir = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&game_dir) else {
            continue;
        };
        if metadata_is_link(&metadata) || !metadata.is_dir() {
            continue;
        }
        let Some(game_id) = game_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if validate_game_id(game_id).is_err() {
            continue;
        }
        let Ok(_lock) = acquire_game_lock(&game_dir) else {
            continue;
        };
        if let Err(error) = recover_pending_journals(&game_dir, Some(game_id)) {
            tracing::error!(game_id, %error, "SQOBA startup recovery requires manual recovery");
        }
    }
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), SqobaError> {
    let parent = path
        .parent()
        .ok_or_else(|| SqobaError::InvalidArchive("journal has no parent".into()))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(value)?)?;
        file.sync_all()?;
        atomic_replace(&temporary, path)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

fn sync_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(windows)]
fn atomic_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let from = from
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let to = to
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
fn atomic_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

/// Resolve `backup_id` → path. id может быть либо `SqobaBackup.id` (uuid из meta),
/// либо file-stem (timestamp). Возвращает `None` если не найден.
pub fn resolve_backup_path(game_id: &str, backup_id: &str) -> Option<PathBuf> {
    resolve_backup_path_with_root(game_id, backup_id, &dest_root())
}

pub fn resolve_backup_path_with_root(
    game_id: &str,
    backup_id: &str,
    dest_root: &Path,
) -> Option<PathBuf> {
    if validate_game_id(game_id).is_err() {
        return None;
    }
    let backups = list_backups_with_root(game_id, dest_root);
    for b in backups {
        if b.id == backup_id {
            return Some(b.dest_path);
        }
        if let Some(stem) = b.dest_path.file_stem().and_then(|s| s.to_str()) {
            if stem == backup_id {
                return Some(b.dest_path);
            }
        }
    }
    None
}

// ───────────────────────── tests ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn mk_file(path: &Path, content: &[u8]) {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn sqoba_discover_save_paths_filters_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let exists = tmp.path().join("exists");
        let missing = tmp.path().join("missing");
        fs::create_dir_all(&exists).unwrap();
        let manual = vec![exists.clone(), missing.clone()];
        let got = discover_save_paths("AnyName", Some(&manual));
        assert_eq!(got, vec![exists]);
    }

    #[test]
    fn sqoba_discover_save_paths_empty_when_no_match() {
        // Не задаём manual override; имя достаточно случайное чтобы не совпало
        // ни с одним env path реального хоста.
        let got = discover_save_paths("__definitely_no_such_game_xyzzy__", None);
        assert!(got.is_empty(), "got = {:?}", got);
    }

    #[test]
    fn sqoba_backup_zips_files() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("save1.dat"), b"hello world");
        mk_file(&src.join("sub/save2.dat"), b"nested content");

        let dest_root = tmp.path().join("backups");
        let manual = vec![src.clone()];
        let b = backup_with_root("g1", "GameOne", Some(&manual), &dest_root, 10).unwrap();
        assert_eq!(b.files_count, 2);
        assert!(b.bytes > 0);
        assert!(b.dest_path.exists());

        // Verify zip content by extracting via restore into a fresh dir.
        // Точечный тест: открываем zip и проверяем что нужные имена есть.
        let f = fs::File::open(&b.dest_path).unwrap();
        let mut zr = zip::ZipArchive::new(f).unwrap();
        let mut names: Vec<String> = (0..zr.len())
            .map(|i| zr.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert!(names.iter().any(|n| n.ends_with("save1.dat")));
        assert!(names.iter().any(|n| n.ends_with("save2.dat")));
        assert!(names.iter().any(|n| n == META_FILENAME));
    }

    #[test]
    fn sqoba_restore_overwrites_originals() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("a.txt"), b"ORIGINAL");
        mk_file(&src.join("sub/b.txt"), b"NESTED-ORIG");

        let dest_root = tmp.path().join("backups");
        let manual = vec![src.clone()];
        let b = backup_with_root("g2", "GameTwo", Some(&manual), &dest_root, 10).unwrap();

        // Mutate originals.
        fs::write(src.join("a.txt"), b"MODIFIED").unwrap();
        fs::write(src.join("sub/b.txt"), b"MODIFIED-NESTED").unwrap();

        let res = restore(&b.dest_path).unwrap();
        assert_eq!(res.status, RestoreStatus::Committed);
        assert_eq!(res.restored_files, 2);
        assert!(res.errors.is_empty(), "errors = {:?}", res.errors);
        assert!(res
            .recovery_backup
            .as_ref()
            .is_some_and(|path| path.exists()));
        assert_eq!(fs::read(src.join("a.txt")).unwrap(), b"ORIGINAL");
        assert_eq!(fs::read(src.join("sub/b.txt")).unwrap(), b"NESTED-ORIG");
    }

    #[test]
    fn sqoba_restore_recreates_missing_source_root() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("save.dat"), b"ORIGINAL");
        let dest_root = tmp.path().join("backups");
        let backup = backup_with_root(
            "g-missing",
            "GameMissing",
            Some(std::slice::from_ref(&src)),
            &dest_root,
            10,
        )
        .unwrap();

        fs::remove_dir_all(&src).unwrap();
        let result = restore(&backup.dest_path).unwrap();

        assert_eq!(result.status, RestoreStatus::Committed);
        assert_eq!(fs::read(src.join("save.dat")).unwrap(), b"ORIGINAL");
    }

    #[test]
    fn sqoba_backup_rejects_destination_inside_save_root() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("save.dat"), b"ORIGINAL");
        let dest_root = src.join("backups");

        let error =
            backup_with_root("g-overlap", "GameOverlap", Some(&[src]), &dest_root, 10).unwrap_err();

        assert!(error.to_string().contains("overlaps a save root"));
    }

    #[test]
    fn sqoba_list_backups_sorted_by_timestamp_desc() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("s.dat"), b"x");
        let dest_root = tmp.path().join("backups");
        let manual = vec![src.clone()];

        // Создаём три бекапа с искусственно разными timestamp'ами путём переименования
        // (т.к. backup_with_root использует chrono::Utc::now() — в реальности они
        // могут попасть в одну секунду).
        let _ = backup_with_root("g3", "GameThree", Some(&manual), &dest_root, 10).unwrap();
        let game_dir = dest_root.join("g3");
        // Найти созданный zip и переименовать.
        let original: Vec<_> = fs::read_dir(&game_dir).unwrap().flatten().collect();
        assert_eq!(original.len(), 1);
        let orig = original[0].path();

        let p1 = game_dir.join("20200101T000000Z.zip");
        let p2 = game_dir.join("20210101T000000Z.zip");
        let p3 = game_dir.join("20220101T000000Z.zip");
        fs::copy(&orig, &p1).unwrap();
        fs::copy(&orig, &p2).unwrap();
        fs::copy(&orig, &p3).unwrap();
        fs::remove_file(&orig).unwrap();

        let list = list_backups_with_root("g3", &dest_root);
        assert_eq!(list.len(), 3);
        assert!(list[0].dest_path.ends_with("20220101T000000Z.zip"));
        assert!(list[1].dest_path.ends_with("20210101T000000Z.zip"));
        assert!(list[2].dest_path.ends_with("20200101T000000Z.zip"));
    }

    #[test]
    fn sqoba_rotation_keeps_n_latest() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("s.dat"), b"x");
        let dest_root = tmp.path().join("backups");
        let manual = vec![src.clone()];

        // Сначала набросим 12 «старых» zip-файлов с фиксированными именами.
        let game_dir = dest_root.join("g4");
        fs::create_dir_all(&game_dir).unwrap();
        // Создадим один реальный zip, чтобы получить корректный архив (восстановление
        // их читать не будет, но rotate_backups смотрит только на расширение).
        let template = backup_with_root("g4", "GameFour", Some(&manual), &dest_root, 100).unwrap();
        for i in 0..12 {
            let p = game_dir.join(format!("2010{:02}01T000000Z.zip", i + 1));
            fs::copy(&template.dest_path, &p).unwrap();
        }
        fs::remove_file(&template.dest_path).unwrap();

        // Теперь делаем real backup с keep=10 → должно остаться ровно 10.
        let _ = backup_with_root("g4", "GameFour", Some(&manual), &dest_root, 10).unwrap();

        let zips: Vec<_> = fs::read_dir(&game_dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("zip"))
            .collect();
        assert_eq!(
            zips.len(),
            10,
            "files = {:?}",
            zips.iter().map(|e| e.file_name()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn sqoba_resolve_backup_path_by_id_and_stem() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("s.dat"), b"x");
        let dest_root = tmp.path().join("backups");
        let manual = vec![src.clone()];
        let b = backup_with_root("g5", "GameFive", Some(&manual), &dest_root, 10).unwrap();

        let by_id = resolve_backup_path_with_root("g5", &b.id, &dest_root).unwrap();
        assert_eq!(by_id, b.dest_path);

        let stem = b
            .dest_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let by_stem = resolve_backup_path_with_root("g5", &stem, &dest_root).unwrap();
        assert_eq!(by_stem, b.dest_path);

        assert!(resolve_backup_path_with_root("g5", "no-such", &dest_root).is_none());
    }

    #[test]
    fn sqoba_backup_returns_no_save_paths_when_none() {
        let tmp = TempDir::new().unwrap();
        let dest_root = tmp.path().join("backups");
        let manual: Vec<PathBuf> = vec![tmp.path().join("does-not-exist")];
        let err = backup_with_root("g6", "GameSix", Some(&manual), &dest_root, 10).unwrap_err();
        assert!(matches!(
            err,
            SqobaError::NoSavePathsFound { game_id } if game_id == "g6"
        ));
    }

    #[test]
    fn sqoba_restore_rejects_absolute_path_in_zip() {
        use std::io::Write as IoWrite;
        use zip::write::{FileOptions, ZipWriter};

        let tmp = TempDir::new().unwrap();
        let archive_dir = tmp.path().join("evil");
        fs::create_dir_all(&archive_dir).unwrap();
        let zip_path = archive_dir.join("evil.zip");
        let games_dir = tmp.path().join("games");
        fs::create_dir_all(&games_dir).unwrap();

        // Craft a zip with an absolute-path entry and a valid meta.
        // The absolute entry should be rejected; restore must not write outside root.
        let meta_json = serde_json::json!({
            "format_version": SQOBA_FORMAT_VERSION,
            "id": "evil-backup",
            "game_id": "evil",
            "timestamp": "20260101T000000Z",
            "source_root_count": 1,
            "files_count": 2,
            "bytes": 22,
            "source_paths": [games_dir]
        })
        .to_string();

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zw = ZipWriter::new(file);
        let opts = FileOptions::default();

        // Write valid meta.
        zw.start_file(META_FILENAME, opts).unwrap();
        zw.write_all(meta_json.as_bytes()).unwrap();

        // Write a legitimate entry (relative path).
        zw.start_file("source_0/save.dat", opts).unwrap();
        zw.write_all(b"safe content").unwrap();

        // Write a malicious entry with absolute path in the relative part.
        // Entry name format: "source_<idx>/<rel>", so rel = "/evil.txt" is absolute.
        zw.start_file("source_0//evil.txt", opts).unwrap();
        zw.write_all(b"evil content").unwrap();

        zw.finish().unwrap();

        let result = restore(&zip_path).unwrap();

        // The absolute-path entry must be in errors, not restored.
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("unsafe archive path") || e.contains("absolute archive path")),
            "expected rejection error, got errors={:?}",
            result.errors
        );
        // The evil file must NOT exist at the absolute path.
        // On Unix, Path::new("").join("source_0").join("/evil.txt") = /evil.txt
        let evil_path = tmp.path().join("evil.txt");
        assert!(!evil_path.exists(), "/evil.txt was written outside root");
        assert!(
            !std::path::Path::new("/evil.txt").exists(),
            "absolute /evil.txt was written"
        );

        assert_eq!(result.status, RestoreStatus::RolledBack);
        assert_eq!(result.restored_files, 0);
        assert!(!games_dir.join("save.dat").exists());
    }

    #[test]
    fn sqoba_recover_pending_journal_restores_snapshot() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        mk_file(&src.join("save.dat"), b"ORIGINAL");
        let game_dir = tmp.path().join("backups").join("g7");
        let recovery_dir = game_dir.join(RECOVERY_DIR);
        fs::create_dir_all(&recovery_dir).unwrap();
        let snapshot =
            create_verified_backup("g7", &[src.canonicalize().unwrap()], &recovery_dir).unwrap();
        let backup =
            create_verified_backup("g7", &[src.canonicalize().unwrap()], &game_dir).unwrap();
        fs::write(src.join("save.dat"), b"MIXED").unwrap();
        let journal = RestoreJournal {
            version: SQOBA_FORMAT_VERSION,
            game_id: "g7".into(),
            backup_id: backup.id.clone(),
            backup_path: backup.dest_path,
            snapshot_path: snapshot.dest_path,
            staging_dir: recovery_dir.join(".staging-crashed"),
            entries: vec![JournalEntry {
                target: src.canonicalize().unwrap().join("save.dat"),
                snapshot_member: "source_0/save.dat".into(),
                was_present: true,
            }],
        };
        let journal_path = recovery_dir.join(format!("{}.journal.json", backup.id));
        write_json_atomic(&journal_path, &journal).unwrap();

        recover_pending_journals(&game_dir, Some("g7")).unwrap();

        assert_eq!(fs::read(src.join("save.dat")).unwrap(), b"ORIGINAL");
        assert!(!journal_path.exists());
    }
}
