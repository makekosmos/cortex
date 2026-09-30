// ---------------------------------------------------------------------------
// Snapshot restore (KOS-51)
// ---------------------------------------------------------------------------
//
// Atomic restore живой ARK DB из snapshot'а в Core-owned `backups/` директории
// (sibling директория рядом с ark.db, туда же пишет db_backup scheduler).
//
// Гарантии:
//   - caller передаёт только basename `backup_id` — никаких произвольных путей;
//   - источник открывается no-follow (без symlink/reparse) и сначала копируется
//     в staging-файл внутри `backups/` — исходный snapshot не модифицируется и
//     нет TOCTOU на его содержимое между validate и apply;
//   - до изменения live DB: `PRAGMA integrity_check` staging-копии + сравнение
//     нормализованного `sqlite_schema` fingerprint'а (+ `user_version`,
//     `application_id`, provenance `canonical_migration_runs`) с текущей ARK
//     schema;
//   - apply идёт через SQLite Online Backup API прямо в live `Connection`
//     (`conn.restore`) под глобальным DB mutex — destination пишется
//     транзакционно, момента с отсутствующей/частично заменённой primary DB
//     нет;
//   - перед apply снимается pre-restore rollback snapshot; провал
//     post-restore verification откатывает live DB из него. Если и откат
//     не удался, snapshot НЕ удаляется: переименовывается в обычный
//     basename `ark.db.pre-restore-failed-*`, видимый в `db_backup_list`
//     и пригодный для `db_backup_restore` — это последний путь к
//     pre-restore данным (KOS-83).

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use rusqlite::OpenFlags;

/// Записываемая в RPC ответ запись о snapshot'е в `backups/`.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotEntry {
    pub id: String,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_ms: Option<i64>,
}

/// Typed verdict `db_backup_validate`.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotValidation {
    pub id: String,
    pub exists: bool,
    pub integrity_ok: bool,
    pub schema_match: bool,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Typed result `db_backup_restore`. `objects`/`links` — post-restore counts,
/// чтобы caller (Cortex) мог показать, что именно восстановлено, до reload UI.
#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub id: String,
    pub restored: bool,
    pub objects: i64,
    pub links: i64,
}

/// `<dir(ark.db)>/backups` — единственная директория, из которой Core
/// принимает snapshot'ы на restore.
pub fn backups_dir(db_path: &str) -> PathBuf {
    let db = Path::new(db_path);
    match db.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(parent) => parent.join("backups"),
        None => PathBuf::from("backups"),
    }
}

/// `backup_id` обязан быть чистым basename'ом: ровно один `Component::Normal`,
/// без `..`, разделителей и точечных (staging/hidden) имён.
fn validate_snapshot_id(id: &str) -> Result<(), String> {
    // Reject separators explicitly: Path::components() treats `\` as a normal
    // character on Unix, so the component check alone is platform-dependent.
    if id.is_empty()
        || id.starts_with('.')
        || id.contains('/')
        || id.contains('\\')
        || id.contains(':')
    {
        return Err(format!("invalid snapshot id: {id:?}"));
    }
    let mut components = Path::new(id).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) if name == std::ffi::OsStr::new(id) => Ok(()),
        _ => Err(format!("invalid snapshot id: {id:?}")),
    }
}

/// Разрешённый путь snapshot'а: `<backups>/<basename>` + проверка, что
/// canonical parent — это именно Core-owned backups dir.
fn resolve_snapshot_path(db_path: &str, id: &str) -> Result<PathBuf, String> {
    validate_snapshot_id(id)?;
    let dir = backups_dir(db_path);
    let candidate = dir.join(id);
    if !candidate.exists() {
        return Err(format!("snapshot not found: {id}"));
    }
    let dir_canonical = fs::canonicalize(&dir)
        .map_err(|e| format!("backups dir is not accessible: {e}"))?;
    // Canonicalize самого файла: leaf-symlink наружу из backups dir
    // резолвится в чужой parent и отклоняется (no-follow open ниже —
    // вторая линия защиты от TOCTOU).
    let resolved = fs::canonicalize(&candidate)
        .map_err(|e| format!("snapshot path is not resolvable: {e}"))?;
    if resolved.parent() != Some(dir_canonical.as_path()) {
        return Err(format!("snapshot escapes backups dir: {id}"));
    }
    Ok(candidate)
}

/// Открывает файл без следования symlink/reparse point. На Windows —
/// `FILE_FLAG_OPEN_REPARSE_POINT`: если путь оказался reparse point'ом,
/// handle ссылается на саму ссылку, и post-open check её отклоняет.
/// На прочих платформах — `symlink_metadata` pre-check + post-open verify
/// через metadata открытого handle.
fn open_no_follow(path: &Path) -> Result<fs::File, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("snapshot metadata failed: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("snapshot is not a regular file".to_string());
    }
    open_no_follow_handle(path)
}

#[cfg(windows)]
fn open_no_follow_handle(path: &Path) -> Result<fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|e| format!("snapshot open failed: {e}"))?;
    // Reparse point открывается как сама ссылка — отклоняем.
    let opened = file
        .metadata()
        .map_err(|e| format!("snapshot metadata failed: {e}"))?;
    if opened.file_type().is_symlink() || !opened.file_type().is_file() {
        return Err("snapshot is a reparse point".to_string());
    }
    Ok(file)
}

#[cfg(not(windows))]
fn open_no_follow_handle(path: &Path) -> Result<fs::File, String> {
    let file = fs::File::open(path).map_err(|e| format!("snapshot open failed: {e}"))?;
    let opened = file
        .metadata()
        .map_err(|e| format!("snapshot metadata failed: {e}"))?;
    if !opened.file_type().is_file() {
        return Err("snapshot is not a regular file".to_string());
    }
    Ok(file)
}

/// Копирует snapshot в staging-файл внутри `backups/` через no-follow open.
/// Restore и validate работают только с staging копией: исходник не
/// модифицируется, а его содержимое frozen на момент копирования.
fn stage_snapshot(db_path: &str, id: &str) -> Result<PathBuf, String> {
    let src_path = resolve_snapshot_path(db_path, id)?;
    let mut src = open_no_follow(&src_path)?;
    let staging = backups_dir(db_path).join(format!(
        ".restore-src-{:016x}.db",
        rand::random::<u64>()
    ));
    let mut dst = fs::File::create(&staging)
        .map_err(|e| format!("staging create failed: {e}"))?;
    let copied = io::copy(&mut src, &mut dst)
        .map_err(|e| format!("snapshot staging copy failed: {e}"));
    drop(dst);
    if let Err(e) = copied {
        let _ = fs::remove_file(&staging);
        return Err(e);
    }
    Ok(staging)
}

fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Нормализованный fingerprint схемы: все записи `sqlite_schema`
/// (type/name/tbl_name/sql со схлопнутым whitespace), `user_version`,
/// `application_id` и provenance-строки `canonical_migration_runs`.
/// Snapshot допустим к restore только при полном совпадении с live DB.
pub fn schema_fingerprint(conn: &Connection) -> Result<Vec<String>, String> {
    let mut rows: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT type, name, tbl_name, COALESCE(sql, '')
                 FROM sqlite_schema ORDER BY type, name, tbl_name",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(format!(
                    "schema|{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    normalize_sql(&row.get::<_, String>(3)?),
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows
    };
    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let application_id: i64 = conn
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.push(format!("pragma|user_version|{user_version}"));
    rows.push(format!("pragma|application_id|{application_id}"));
    let has_migration_runs: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema
             WHERE type='table' AND name='canonical_migration_runs')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if has_migration_runs {
        let mut stmt = conn
            .prepare(
                "SELECT contract_version, status, source_inventory_hash
                 FROM canonical_migration_runs ORDER BY contract_version",
            )
            .map_err(|e| e.to_string())?;
        let mut runs = stmt
            .query_map([], |row| {
                Ok(format!(
                    "migration|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows.append(&mut runs);
    }
    Ok(rows)
}

/// Открывает staging-копию read-only и проверяет integrity + schema match
/// против live conn.
fn validate_staged(
    staging: &Path,
    live: &Connection,
) -> Result<(bool, bool, Option<String>), String> {
    let src = match Connection::open_with_flags(staging, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(e) => return Ok((false, false, Some(format!("open failed: {e}")))),
    };
    if let Err(e) = check_integrity(&src) {
        return Ok((false, false, Some(e)));
    }
    let src_fp = schema_fingerprint(&src)?;
    let live_fp = schema_fingerprint(live)?;
    if src_fp != live_fp {
        return Ok((
            true,
            false,
            Some("schema mismatch with live ARK DB".to_string()),
        ));
    }
    Ok((true, true, None))
}

/// `db_backup_list`: только regular files из Core-owned `backups/`.
pub fn list_snapshots(db_path: &str) -> Result<Vec<SnapshotEntry>, String> {
    let dir = backups_dir(db_path);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(id) = name.to_str() else { continue };
        if validate_snapshot_id(id).is_err() {
            continue;
        }
        // file_type() не следует symlink'ам — ссылки не попадают в list.
        let file_type = entry
            .file_type()
            .map_err(|e| format!("snapshot metadata failed: {e}"))?;
        if !file_type.is_file() {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|e| format!("snapshot metadata failed: {e}"))?;
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        entries.push(SnapshotEntry {
            id: id.to_string(),
            size_bytes: meta.len(),
            modified_ms,
        });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

/// `db_backup_validate`: dry-run проверки snapshot'а против live DB.
/// Live DB не модифицируется.
pub fn validate_snapshot(
    conn: &Connection,
    db_path: &str,
    id: &str,
) -> Result<SnapshotValidation, String> {
    let mut result = SnapshotValidation {
        id: id.to_string(),
        exists: false,
        integrity_ok: false,
        schema_match: false,
        valid: false,
        error: None,
    };
    match resolve_snapshot_path(db_path, id) {
        Ok(_) => result.exists = true,
        Err(e) => {
            result.error = Some(e);
            return Ok(result);
        }
    }
    let staging = stage_snapshot(db_path, id)?;
    let staged = validate_staged(&staging, conn);
    let _ = fs::remove_file(&staging);
    let (integrity_ok, schema_match, error) = staged?;
    result.integrity_ok = integrity_ok;
    result.schema_match = schema_match;
    result.error = error;
    result.valid = integrity_ok && schema_match;
    Ok(result)
}

/// Seam для тестов: `(live_conn, expected_snapshot_fingerprint)`.
type PostVerify = dyn Fn(&Connection, &[String]) -> Result<(), String>;

/// Seam для тестов: `(live_conn, rollback_snapshot_path)` — откат live DB
/// из pre-restore snapshot'а.
type RollbackRestore = dyn Fn(&mut Connection, &Path) -> Result<(), String>;

/// Post-restore verification по умолчанию: integrity live DB + schema
/// fingerprint совпадает с fingerprint'ом применённого snapshot'а.
fn default_post_verify(conn: &Connection, expected: &[String]) -> Result<(), String> {
    check_integrity(conn)?;
    let actual = schema_fingerprint(conn)?;
    if actual != expected {
        return Err("post-restore schema fingerprint mismatch".to_string());
    }
    Ok(())
}

/// Откат по умолчанию: Online Backup из rollback-файла обратно в live conn.
fn default_rollback_restore(conn: &mut Connection, path: &Path) -> Result<(), String> {
    conn.restore(
        rusqlite::DatabaseName::Main,
        path,
        None::<fn(rusqlite::backup::Progress)>,
    )
    .map_err(|e| e.to_string())
}

include!("snapshot/part02.rs");
