// Periodic DB backup scheduler.
//
// Делает SQLite Online Backup ARK базы в `<data_dir>/backups/ark.db.backup-YYYY-MM-DD-HHMMSS`
// раз в N часов (default 24, override через MUNDUS_BACKUP_INTERVAL_HOURS).
// Online Backup API не блокирует concurrent readers/writer — safe для live DB.
//
// Retention (см. `retention` модуль, KOS-300): после успешного backup'а и на
// каждом старте Engine держим последние `MUNDUS_BACKUP_RETAIN_COUNT`
// (default 7) backup'ов, убираем orphan SQLite sidecar'ы и stale temp файлы
// crashed restore.
//
// Last-backup timestamp хранится в sync_kv под ключом `mundus.last_backup_ts`.
// Если backup упал — логируем eprintln и продолжаем; следующий запуск
// попробует снова. Failure не должен блокировать backend startup.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::ark_host::ArkHost;

mod retention;

const SYNC_KV_LAST_BACKUP: &str = "mundus.last_backup_ts";
const BACKUPS_SUBDIR: &str = "backups";
const BACKUP_FILE_PREFIX: &str = "ark.db.backup-";
static BACKUP_ACTIVE: AtomicBool = AtomicBool::new(false);
static BACKUP_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn interval_hours() -> u64 {
    std::env::var("MUNDUS_BACKUP_INTERVAL_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(24)
}

fn retain_count() -> usize {
    std::env::var("MUNDUS_BACKUP_RETAIN_COUNT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(7)
}

fn backup_filename(now: DateTime<Utc>) -> String {
    format!("{}{}", BACKUP_FILE_PREFIX, now.format("%Y-%m-%d-%H%M%S"))
}

fn parse_backup_timestamp(name: &str) -> Option<DateTime<Utc>> {
    let stripped = name.strip_prefix(BACKUP_FILE_PREFIX)?;
    let parsed = chrono::NaiveDateTime::parse_from_str(stripped, "%Y-%m-%d-%H%M%S").ok()?;
    Some(parsed.and_utc())
}

async fn read_last_backup_ts(ark: &ArkHost) -> Option<DateTime<Utc>> {
    let resp = ark
        .request("get_sync_kv", json!({ "key": SYNC_KV_LAST_BACKUP }))
        .await
        .ok()?;
    let value = resp.data.as_str()?;
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

async fn write_last_backup_ts(ark: &ArkHost, ts: DateTime<Utc>) -> Result<(), String> {
    ark.request(
        "set_sync_kv",
        json!({ "key": SYNC_KV_LAST_BACKUP, "value": ts.to_rfc3339() }),
    )
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn backup_wait_timeout_secs() -> u64 {
    std::env::var("MUNDUS_BACKUP_WAIT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(900)
}

fn backup_pages_per_step() -> i32 {
    std::env::var("ARK_BACKUP_PAGES_PER_STEP")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(256)
}

fn backup_pause_ms() -> u64 {
    std::env::var("ARK_BACKUP_PAUSE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
}

pub fn diagnostics_snapshot() -> DbBackupDiagnosticsSnapshot {
    DbBackupDiagnosticsSnapshot {
        active: BACKUP_ACTIVE.load(Ordering::SeqCst),
        pages_per_step: backup_pages_per_step(),
        pause_ms: backup_pause_ms(),
        background_mode: true,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DbBackupDiagnosticsSnapshot {
    pub active: bool,
    pub pages_per_step: i32,
    pub pause_ms: u64,
    pub background_mode: bool,
}

fn next_backup_destination(
    backups_dir: &Path,
    mut now: DateTime<Utc>,
) -> Result<(DateTime<Utc>, PathBuf), String> {
    for _ in 0..86_400 {
        let path = backups_dir.join(backup_filename(now));
        if !path.exists() {
            return Ok((now, path));
        }
        now += chrono::Duration::seconds(1);
    }
    Err("unable to allocate a unique backup filename".to_string())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validated_backups_dir(data_dir: &Path, create: bool) -> Result<Option<PathBuf>, String> {
    if create {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("create data directory {data_dir:?}: {e}"))?;
    }
    let canonical_data_dir = match data_dir.canonicalize() {
        Ok(path) => path,
        Err(error) if !create && error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("canonicalize data directory {data_dir:?}: {error}")),
    };
    let dir = data_dir.join(BACKUPS_SUBDIR);
    if create {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all {dir:?}: {e}"))?;
    }
    let metadata = match std::fs::symlink_metadata(&dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("stat backup directory {dir:?}: {error}")),
    };
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(format!(
            "backup directory must be a real directory: {dir:?}"
        ));
    }
    let canonical_dir = dir
        .canonicalize()
        .map_err(|e| format!("canonicalize backup directory {dir:?}: {e}"))?;
    if !canonical_dir.starts_with(&canonical_data_dir) {
        return Err(format!(
            "backup directory escapes data directory: {canonical_dir:?}"
        ));
    }
    Ok(Some(canonical_dir))
}

/// Дождаться события `db_backup_result` для нашего `dest_str` (backup идёт
/// async в ark-core service). Bounded таймаутом, чтобы не зависнуть навсегда если
/// backup thread умер посреди копирования.
async fn wait_for_backup_result(
    events: &mut tokio::sync::broadcast::Receiver<(String, serde_json::Value)>,
    dest_str: &str,
) -> Result<(), String> {
    use tokio::sync::broadcast::error::RecvError;
    let deadline = std::time::Duration::from_secs(backup_wait_timeout_secs());
    let wait = async {
        loop {
            match events.recv().await {
                Ok((name, payload)) if name == "db_backup_result" => {
                    if payload.get("dest").and_then(|d| d.as_str()) != Some(dest_str) {
                        continue; // чужой backup (не наш dest) — игнор
                    }
                    if payload.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
                        return Ok(());
                    }
                    let err = payload
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    return Err(format!("backup failed: {err}"));
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return Err("event channel closed".to_string()),
            }
        }
    };
    match tokio::time::timeout(deadline, wait).await {
        Ok(res) => res,
        Err(_) => Err("backup timed out waiting for completion".to_string()),
    }
}

/// Ensure `<data_dir>/backups/` exists and return path.
fn ensure_backups_dir(data_dir: &Path) -> Result<PathBuf, String> {
    validated_backups_dir(data_dir, true)?.ok_or("backup directory was not created".to_string())
}

/// Применить retention к `<data_dir>/backups/` (см. retention.rs: ротация +
/// orphan sidecar'ы + stale restore temps). Failure не фатален — логируем.
fn enforce_retention(backups_dir: &Path) {
    match retention::enforce(backups_dir, retain_count()) {
        Ok(report) if report.total_removed() > 0 => eprintln!(
            "[db-backup] retention: {} rotated, {} sidecars, {} pre-restore, {} restore temps \\
                 removed",
            report.rotated_backups,
            report.removed_sidecars,
            report.removed_pre_restore,
            report.removed_restore_temps
        ),
        Ok(_) => {}
        Err(e) => eprintln!("[db-backup] retention failed: {e}"),
    }
}

/// Сделать backup сейчас (без проверки interval). Используется в тестах и
/// при manual trigger через future IPC. Возвращает path созданного файла.
pub async fn run_backup_now(ark: &ArkHost, data_dir: &Path) -> Result<PathBuf, String> {
    let _guard = BACKUP_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let backups_dir = ensure_backups_dir(data_dir)?;
    let (now, dest) = next_backup_destination(&backups_dir, Utc::now())?;
    let dest_str = dest
        .to_str()
        .ok_or("backup dest path is not UTF-8")?
        .to_string();

    // Подписка ДО запроса: backup в ark-core service идёт async на отдельном
    // background-priority потоке, событие завершения может прийти раньше, чем
    // мы успеем подписаться после await'а RPC.
    let mut events = ark.subscribe_events();

    eprintln!("[db-backup] starting → {dest_str}");
    let backup_started = std::time::Instant::now();
    // RPC возвращается сразу ({"started": true}) — копирование продолжается в
    // фоне, не блокируя серийный request-loop сервиса.
    BACKUP_ACTIVE.store(true, Ordering::SeqCst);
    let start_result = ark
        .request("db_backup", json!({ "dest_path": dest_str.clone() }))
        .await
        .map_err(|e| format!("db_backup RPC failed: {e}"));
    if let Err(e) = start_result {
        BACKUP_ACTIVE.store(false, Ordering::SeqCst);
        return Err(e);
    }

    // Ждём событие завершения (или таймаут) — только после успеха пишем
    // last_backup_ts и ротируем (иначе ротация могла бы удалить незавершённый файл).
    let wait_result = wait_for_backup_result(&mut events, &dest_str).await;
    BACKUP_ACTIVE.store(false, Ordering::SeqCst);
    wait_result?;

    write_last_backup_ts(ark, now).await?;

    enforce_retention(&backups_dir);

    eprintln!(
        "[db-backup] success → {dest:?} ({} ms, background-priority chunked copy)",
        backup_started.elapsed().as_millis()
    );
    Ok(dest)
}

// ---------------------------------------------------------------------------
// KOS-77: privileged Core snapshot RPCs (KOS-51: db_backup_list /
// db_backup_validate / db_backup_restore). Manager не трогает файлы сам —
// `<db_dir>/backups/` принадлежит Core; restore идёт Online Backup API в live
// conn под BACKUP_GATE, без момента с отсутствующей primary DB.
// ---------------------------------------------------------------------------

/// `backup_id` — только basename в формате `ark.db.backup-YYYY-MM-DD-HHMMSS`
/// (ровно то, что создаёт `run_backup_now` и показывает список Manager'а).
/// Core повторно проверяет границы (basename-only, no-follow, staging),
/// но этот boundary fail-closed отсекает произвольные строки до RPC.
fn snapshot_id(params: &serde_json::Value) -> Result<&str, String> {
    let id = params
        .get("backup_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if parse_backup_timestamp(id).is_some() {
        Ok(id)
    } else {
        Err("backup_id must be an ark.db backup filename".to_string())
    }
}

async fn core_snapshot_request(
    ark: &ArkHost,
    operation: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let response = ark
        .request(operation, params)
        .await
        .map_err(|e| format!("{operation} RPC failed: {e}"))?;
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| format!("{operation} failed")));
    }
    Ok(response.data)
}

async fn list_snapshots(ark: &ArkHost) -> Result<serde_json::Value, String> {
    core_snapshot_request(ark, "db_backup_list", json!({})).await
}

async fn validate_snapshot(
    ark: &ArkHost,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = snapshot_id(params)?;
    core_snapshot_request(ark, "db_backup_validate", json!({ "backup_id": id })).await
}

/// Атомарный restore через Core. После успеха публикует `db_restored` всем
/// WS клиентам (desktop shell, host apps): их in-memory состояние устарело,
/// они обязаны перечитать ARK.
async fn restore_snapshot(
    ark: &ArkHost,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = snapshot_id(params)?;
    let report =
        core_snapshot_request(ark, "db_backup_restore", json!({ "backup_id": id })).await?;
    let mut event = serde_json::Map::new();
    event.insert("event".to_string(), json!("db_restored"));
    if let Some(fields) = report.as_object() {
        event.extend(fields.clone());
    }
    ark.emit_event(serde_json::Value::Object(event));
    Ok(report)
}

/// `manager.db_backups.*` — единая точка входа из dispatch_standard.
/// list/validate/restore уходят в Core; create остаётся на scheduler'е выше.
pub async fn handle_manager_op(
    op: &str,
    ark: &ArkHost,
    data_dir: &Path,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match op {
        "db_backups.list" => list_snapshots(ark).await,
        "db_backups.create" => run_backup_now(ark, data_dir)
            .await
            .map(|path| json!({ "path": path })),
        "db_backups.validate" => validate_snapshot(ark, params).await,
        "db_backups.restore" => restore_snapshot(ark, params).await,
        _ => Err(format!("manager.{op}: unknown-operation")),
    }
}

/// Проверить когда был последний backup, и если прошло >= interval —
/// сделать backup. Вызывается на старте backend (не как periodic timer —
/// backend каждый restart дёргает; для long-running daemon процессов
/// можно добавить отдельный tick'нутый scheduler позже).
pub async fn maybe_backup_on_startup(ark: Arc<ArkHost>, data_dir: PathBuf) {
    // Retention применяем на каждом старте, а не только когда backup due:
    // между интервалами orphan sidecar'ы и stale restore temps копились бы
    // бесконечно. Тот же BACKUP_LOCK, что у run_backup_now — cleanup не
    // гонится с ручным db_backups.create. Директорию не создаём: чистить
    // несуществующее нечего.
    match validated_backups_dir(&data_dir, false) {
        Ok(Some(backups_dir)) => {
            let _guard = BACKUP_LOCK
                .get_or_init(|| tokio::sync::Mutex::new(()))
                .lock()
                .await;
            enforce_retention(&backups_dir);
        }
        Ok(None) => {}
        Err(e) => eprintln!("[db-backup] startup retention skipped: {e}"),
    }

    let interval = chrono::Duration::hours(interval_hours() as i64);
    let last = read_last_backup_ts(&ark).await;
    let should_backup = match last {
        Some(ts) => {
            let elapsed = Utc::now().signed_duration_since(ts);
            if elapsed >= interval {
                eprintln!(
                    "[db-backup] last backup was {:.1}h ago (interval {}h), running",
                    elapsed.num_minutes() as f64 / 60.0,
                    interval_hours()
                );
                true
            } else {
                eprintln!(
                    "[db-backup] last backup was {:.1}h ago (interval {}h), skipping",
                    elapsed.num_minutes() as f64 / 60.0,
                    interval_hours()
                );
                false
            }
        }
        None => {
            eprintln!("[db-backup] no previous backup recorded, running");
            true
        }
    };
    if !should_backup {
        return;
    }
    if let Err(e) = run_backup_now(&ark, &data_dir).await {
        eprintln!("[db-backup] failed: {e}");
    }
}

#[cfg(test)]
mod tests;
