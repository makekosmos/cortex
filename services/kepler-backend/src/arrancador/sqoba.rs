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
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::arrancador::config as ar_config;

const DEFAULT_KEEP_BACKUPS: u32 = 10;
const META_FILENAME: &str = "_sqoba_meta.json";

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub restored_files: u32,
    pub bytes: u64,
    pub errors: Vec<String>,
}

/// Метаданные, хранящиеся внутри zip как `_sqoba_meta.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZipMeta {
    id: String,
    game_id: String,
    timestamp: String,
    /// Маппинг: per-source index (`source_0/...` etc внутри zip) → absolute path.
    source_paths: Vec<PathBuf>,
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
    let sources = discover_save_paths(game_name, manual_paths);
    if sources.is_empty() {
        return Err(SqobaError::NoSavePathsFound {
            game_id: game_id.to_string(),
        });
    }

    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

    let game_dir = dest_root.join(game_id);
    std::fs::create_dir_all(&game_dir)?;
    let dest_path = game_dir.join(format!("{}.zip", timestamp));

    let file = std::fs::File::create(&dest_path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut files_count: u32 = 0;
    let mut bytes: u64 = 0;

    for (idx, src) in sources.iter().enumerate() {
        let prefix = format!("source_{}", idx);
        zw.add_directory(&prefix, opts)?;
        write_dir_recursive(&mut zw, src, src, &prefix, opts, &mut files_count, &mut bytes)?;
    }

    let meta = ZipMeta {
        id: id.clone(),
        game_id: game_id.to_string(),
        timestamp: timestamp.clone(),
        source_paths: sources.clone(),
    };
    zw.start_file(META_FILENAME, opts)?;
    let meta_json = serde_json::to_vec_pretty(&meta)?;
    zw.write_all(&meta_json)?;

    zw.finish()?;

    rotate_backups(&game_dir, keep)?;

    Ok(SqobaBackup {
        id,
        game_id: game_id.to_string(),
        timestamp,
        dest_path,
        files_count,
        bytes,
        source_paths: sources,
    })
}

fn write_dir_recursive<W: Write + std::io::Seek>(
    zw: &mut zip::ZipWriter<W>,
    root: &Path,
    cur: &Path,
    prefix: &str,
    opts: zip::write::FileOptions,
    files_count: &mut u32,
    bytes: &mut u64,
) -> Result<(), SqobaError> {
    for entry in std::fs::read_dir(cur)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let zip_name = format!("{}/{}", prefix, rel_str);
        let ftype = entry.file_type()?;
        if ftype.is_dir() {
            zw.add_directory(&zip_name, opts)?;
            write_dir_recursive(zw, root, &path, prefix, opts, files_count, bytes)?;
        } else if ftype.is_file() {
            zw.start_file(&zip_name, opts)?;
            let mut f = std::fs::File::open(&path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            zw.write_all(&buf)?;
            *files_count += 1;
            *bytes += buf.len() as u64;
        }
    }
    Ok(())
}

fn rotate_backups(game_dir: &Path, keep: u32) -> Result<(), SqobaError> {
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
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

// ───────────────────────── list ─────────────────────────

pub fn list_backups(game_id: &str) -> Vec<SqobaBackup> {
    list_backups_with_root(game_id, &dest_root())
}

pub fn list_backups_with_root(game_id: &str, dest_root: &Path) -> Vec<SqobaBackup> {
    let game_dir = dest_root.join(game_id);
    if !game_dir.exists() {
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
        if let Some(b) = inspect_backup(game_id, &path) {
            out.push(b);
        }
    }
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    out
}

fn inspect_backup(game_id: &str, path: &Path) -> Option<SqobaBackup> {
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let file = std::fs::File::open(path).ok()?;
    let mut zr = zip::ZipArchive::new(file).ok()?;

    // Попробовать прочитать meta. Timestamp всегда берём из имени файла —
    // оно по convention и есть ISO timestamp, и оно стабильно сортируется
    // лексикографически.
    let (id, source_paths) = match zr.by_name(META_FILENAME) {
        Ok(mut zf) => {
            let mut buf = String::new();
            if zf.read_to_string(&mut buf).is_ok() {
                if let Ok(meta) = serde_json::from_str::<ZipMeta>(&buf) {
                    (meta.id, meta.source_paths)
                } else {
                    (file_stem.clone(), Vec::new())
                }
            } else {
                (file_stem.clone(), Vec::new())
            }
        }
        Err(_) => (file_stem.clone(), Vec::new()),
    };
    let timestamp = file_stem.clone();

    // Count non-meta file entries.
    let mut files_count: u32 = 0;
    for i in 0..zr.len() {
        if let Ok(zf) = zr.by_index(i) {
            if zf.is_file() && zf.name() != META_FILENAME {
                files_count += 1;
            }
        }
    }

    Some(SqobaBackup {
        id,
        game_id: game_id.to_string(),
        timestamp,
        dest_path: path.to_path_buf(),
        files_count,
        bytes: size,
        source_paths,
    })
}

// ───────────────────────── restore ─────────────────────────

pub fn restore(backup_path: &Path) -> Result<RestoreResult, SqobaError> {
    let file = std::fs::File::open(backup_path)?;
    let mut zr = zip::ZipArchive::new(file)?;

    // Load meta first to find original source roots.
    let meta: ZipMeta = {
        let mut zf = zr
            .by_name(META_FILENAME)
            .map_err(|_| SqobaError::BackupNotFound(backup_path.display().to_string()))?;
        let mut buf = String::new();
        zf.read_to_string(&mut buf)?;
        serde_json::from_str(&buf)?
    };

    let mut restored_files: u32 = 0;
    let mut bytes: u64 = 0;
    let mut errors: Vec<String> = Vec::new();

    let total = zr.len();
    for i in 0..total {
        let mut zf = match zr.by_index(i) {
            Ok(z) => z,
            Err(e) => {
                errors.push(format!("entry {}: {}", i, e));
                continue;
            }
        };
        if zf.is_dir() {
            continue;
        }
        let name = zf.name().to_string();
        if name == META_FILENAME {
            continue;
        }
        // Expect `source_<idx>/<rel...>`.
        let (idx_str, rel) = match name.split_once('/') {
            Some((a, b)) => (a, b),
            None => {
                errors.push(format!("malformed entry name: {}", name));
                continue;
            }
        };
        let idx: usize = match idx_str.strip_prefix("source_").and_then(|s| s.parse().ok()) {
            Some(n) => n,
            None => {
                errors.push(format!("unknown entry root: {}", idx_str));
                continue;
            }
        };
        let root = match meta.source_paths.get(idx) {
            Some(p) => p.clone(),
            None => {
                errors.push(format!("source_paths[{}] missing in meta", idx));
                continue;
            }
        };
        // Reject path traversal.
        if rel.contains("..") {
            errors.push(format!("rejected path traversal: {}", name));
            continue;
        }
        let target = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = target.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(format!("mkdir {}: {}", parent.display(), e));
                continue;
            }
        }
        let mut buf = Vec::new();
        if let Err(e) = zf.read_to_end(&mut buf) {
            errors.push(format!("read {}: {}", name, e));
            continue;
        }
        match std::fs::write(&target, &buf) {
            Ok(()) => {
                restored_files += 1;
                bytes += buf.len() as u64;
            }
            Err(e) => errors.push(format!("write {}: {}", target.display(), e)),
        }
    }

    Ok(RestoreResult {
        restored_files,
        bytes,
        errors,
    })
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
        assert_eq!(res.restored_files, 2);
        assert!(res.errors.is_empty(), "errors = {:?}", res.errors);
        assert_eq!(fs::read(src.join("a.txt")).unwrap(), b"ORIGINAL");
        assert_eq!(fs::read(src.join("sub/b.txt")).unwrap(), b"NESTED-ORIG");
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
        let template =
            backup_with_root("g4", "GameFour", Some(&manual), &dest_root, 100).unwrap();
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
        assert_eq!(zips.len(), 10, "files = {:?}", zips.iter().map(|e| e.file_name()).collect::<Vec<_>>());
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

        let stem = b.dest_path.file_stem().unwrap().to_string_lossy().to_string();
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
        match err {
            SqobaError::NoSavePathsFound { game_id } => assert_eq!(game_id, "g6"),
            other => panic!("expected NoSavePathsFound, got {:?}", other),
        }
    }
}
