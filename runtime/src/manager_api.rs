use crate::ark_host::ArkHost;
use crate::diagnostics::SharedRpcDiagnostics;
use crate::engine_settings;
use crate::package_service::PackageService;
use crate::protocol_usage::ProtocolUsageStore;
use crate::usage_tracker::UsageTrackerDiagnosticsState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const MAX_LOG_TAIL: usize = 200;
const MAX_BROWSE_PAGE: usize = 200;
const MAX_SEARCH_RESULTS: usize = 100;
const CANONICAL_VERSION: &str = "1.0.0";
const CANONICAL_TYPES: &[(&str, &str, &str)] = &[
    ("com.kosmos.note", "Заметка", "note_obj"),
    ("com.kosmos.task", "Задача", "task_obj"),
    ("com.kosmos.game", "Игра", "game_obj"),
];

fn canonical_identity(type_id: &str) -> Option<(&'static str, &'static str)> {
    CANONICAL_TYPES.iter().find_map(|(canonical, _, alias)| {
        (*canonical == type_id || *alias == type_id).then_some((*canonical, CANONICAL_VERSION))
    })
}

fn canonical_type_sources(types: &[Value]) -> Vec<(&'static str, &'static str, String)> {
    let mut sources = HashMap::new();
    for value in types {
        let Some(id) = value.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some((canonical_id, _)) = canonical_identity(id) else {
            continue;
        };
        if id == canonical_id || !sources.contains_key(canonical_id) {
            sources.insert(canonical_id, id.to_owned());
        }
    }
    CANONICAL_TYPES
        .iter()
        .filter_map(|(canonical_id, _, _)| {
            sources
                .remove(canonical_id)
                .map(|source| (*canonical_id, CANONICAL_VERSION, source))
        })
        .collect()
}

#[derive(Clone)]
pub struct ManagerState {
    data_dir: Arc<PathBuf>,
    bundles: Arc<Mutex<HashMap<String, BundleEntry>>>,
}

struct BundleEntry {
    path: PathBuf,
    expires_at: Instant,
}

impl ManagerState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir: Arc::new(data_dir),
            bundles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub async fn data_summary(
        &self,
        ark: &Arc<ArkHost>,
        package_store: &Path,
    ) -> Result<Value, String> {
        let types = ark
            .request("list_object_types", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        let mut total = 0_u64;
        for (canonical_id, version, id) in
            canonical_type_sources(types.data.as_array().map(Vec::as_slice).unwrap_or_default())
        {
            let rows = ark
                .request("list_object_summaries_by_type", json!({"type_id": id}))
                .await
                .map_err(|e| e.to_string())?;
            let live: Vec<&Value> = rows
                .data
                .as_array()
                .map(|a| a.iter().filter(|row| is_live_public_row(row)).collect())
                .unwrap_or_default();
            let logical_bytes = live
                .iter()
                .map(|row| serde_json::to_vec(row).map(|v| v.len() as u64).unwrap_or(0))
                .sum::<u64>();
            total = total.saturating_add(logical_bytes);
            let name = CANONICAL_TYPES
                .iter()
                .find(|(candidate, _, _)| *candidate == canonical_id)
                .map(|(_, name, _)| *name)
                .unwrap_or_default();
            out.push(json!({"id": canonical_id, "type_id": canonical_id, "type_version": version, "name": name, "count": live.len(), "logical_bytes": logical_bytes}));
        }
        Ok(json!({
            "collected_at": chrono::Utc::now().to_rfc3339(),
            "types": out,
            "logical_bytes": total,
            "managed_storage_bytes": managed_bytes(&self.data_dir, package_store),
        }))
    }

    pub async fn data_types(&self, ark: &Arc<ArkHost>) -> Result<Value, String> {
        let response = ark
            .request("list_object_types", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        let mut types = Vec::new();
        for (canonical_id, version, id) in canonical_type_sources(
            response
                .data
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
        ) {
            let rows = ark
                .request("list_object_summaries_by_type", json!({"type_id": id}))
                .await
                .map_err(|e| e.to_string())?;
            let live = rows
                .data
                .as_array()
                .into_iter()
                .flatten()
                .filter(|row| is_live_public_row(row))
                .collect::<Vec<_>>();
            let logical_bytes = live
                .iter()
                .map(|row| {
                    serde_json::to_vec(row)
                        .map(|bytes| bytes.len() as u64)
                        .unwrap_or(0)
                })
                .sum::<u64>();
            let name = CANONICAL_TYPES
                .iter()
                .find(|(candidate, _, _)| *candidate == canonical_id)
                .map(|(_, name, _)| *name)
                .unwrap_or_default();
            types.push(json!({
                "id": canonical_id,
                "type_id": canonical_id,
                "type_version": version,
                "name": name,
                "count": live.len(),
                "logical_bytes": logical_bytes,
            }));
        }
        Ok(Value::Array(types))
    }

    pub async fn data_list(&self, ark: &Arc<ArkHost>, params: &Value) -> Result<Value, String> {
        let limit = bounded_limit(params.get("limit"), MAX_BROWSE_PAGE)?;
        let offset = parse_cursor(params.get("cursor"))?;
        let type_id = params.get("type_id").and_then(Value::as_str);
        let canonical_type = type_id.and_then(canonical_identity);
        let requested_version = params.get("type_version").and_then(Value::as_str);
        if (type_id.is_some() && canonical_type.is_none())
            || requested_version.is_some_and(|version| version != CANONICAL_VERSION)
        {
            return Err("invalid-type".into());
        }
        let operation = if type_id.is_some() {
            "list_object_summaries_by_type"
        } else {
            "list_object_summaries"
        };
        let request = canonical_type.map_or_else(|| json!({}), |(id, _)| json!({"type_id": id}));
        let response = ark
            .request(operation, request)
            .await
            .map_err(|e| e.to_string())?;
        let rows = response.data.as_array().cloned().unwrap_or_default();
        let filtered = rows.iter().filter(|row| is_live_public_row(row));
        let all: Vec<&Value> = filtered.collect();
        let items: Vec<Value> = all
            .iter()
            .skip(offset)
            .take(limit)
            .filter_map(|row| safe_row(row, None))
            .collect();
        let next_cursor =
            (offset + items.len() < all.len()).then(|| (offset + items.len()).to_string());
        Ok(json!({"items": items, "next_cursor": next_cursor}))
    }

    pub async fn data_search(&self, ark: &Arc<ArkHost>, params: &Value) -> Result<Value, String> {
        let query = params
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if query.is_empty() || query.len() > 256 {
            return Err("invalid-query".into());
        }
        let response = ark
            .request("search_objects", json!({"query": query}))
            .await
            .map_err(|e| e.to_string())?;
        let items = response
            .data
            .as_array()
            .into_iter()
            .flatten()
            .filter(|row| is_live_public_row(row))
            .take(MAX_SEARCH_RESULTS)
            .filter_map(|row| safe_row(row, Some(query)))
            .collect::<Vec<_>>();
        Ok(json!({"items": items, "truncated": items.len() == MAX_SEARCH_RESULTS}))
    }

    pub async fn diagnostics_snapshot(
        &self,
        rpc: &SharedRpcDiagnostics,
        usage: &Arc<UsageTrackerDiagnosticsState>,
        protocol: &Arc<ProtocolUsageStore>,
        packages: &Arc<PackageService>,
    ) -> Value {
        let rpc_snapshot = rpc.snapshot();
        let usage_snapshot = usage.snapshot();
        let protocol_snapshot = protocol.snapshot();
        let workers = packages
            .worker_diagnostics()
            .into_iter()
            .map(|worker| {
                json!({
                    "id": worker.id,
                    "version": worker.version,
                    "status": worker.state,
                    "state": worker.state,
                    "restart_count": worker.restart_count,
                    "generation": worker.generation,
                    "lifecycle_reason": worker.lifecycle_reason,
                    "bridge_status": worker.bridge_status,
                })
            })
            .collect::<Vec<_>>();
        let protocol_usage = json!({
            "tracking_started_at": protocol_snapshot.tracking_started_at,
            "api_v1": protocol_snapshot.api_v1,
            "legacy": protocol_snapshot.legacy,
            "legacy_zero_since": protocol_snapshot.legacy_zero_since,
            "legacy_zero_for_30_days": protocol_snapshot.legacy_zero_for_30_days,
        });
        let legacy_gate = json!({
            "api_v1": {
                "connections": protocol_snapshot.api_v1.connections,
                "last_seen": protocol_snapshot.api_v1.last_seen,
            },
            "legacy": {
                "connections": protocol_snapshot.legacy.connections,
                "last_seen": protocol_snapshot.legacy.last_seen,
            },
            "api_v1_connections": protocol_snapshot.api_v1.connections,
            "legacy_connections": protocol_snapshot.legacy.connections,
            "api_v1_last_seen": protocol_snapshot.api_v1.last_seen,
            "legacy_last_seen": protocol_snapshot.legacy.last_seen,
            "tracking_started_at": protocol_snapshot.tracking_started_at,
            "legacy_zero_since": protocol_snapshot.legacy_zero_since,
            "ready": protocol_snapshot.legacy_zero_for_30_days,
        });
        let components = json!([
            {"name": "rpc", "status": "ok", "state": "ok", "details": rpc_snapshot},
            {"name": "usage_tracker", "status": usage_snapshot.status, "state": usage_snapshot.status, "details": usage_snapshot},
            {"name": "protocol_usage", "status": "ok", "state": "ok", "details": protocol_usage},
        ]);
        json!({"components": components, "workers": workers, "legacy_gate": legacy_gate})
    }

    pub async fn log_tail(&self, rpc: &SharedRpcDiagnostics) -> Value {
        let snapshot = serde_json::to_value(rpc.snapshot()).unwrap_or_else(|_| json!({}));
        let entries = snapshot
            .get("by_operation")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .take(MAX_LOG_TAIL)
                    .map(|(operation, value)| {
                        let line = crate::observability::redact_log_line(
                            &json!({"operation": operation, "stats": value}).to_string(),
                        );
                        serde_json::from_str(&line).unwrap_or_else(|_| json!({}))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        json!({"entries": entries})
    }

    pub async fn create_bundle(&self, snapshot: Value, tail: Value) -> Result<Value, String> {
        let dir = self.data_dir.join("support-bundles");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let handle = uuid::Uuid::new_v4().to_string();
        let path = dir.join(format!("{handle}.json"));
        let bundle = json!({"format_version": 1, "snapshot": snapshot, "log_tail": tail});
        let redacted = crate::observability::redact_log_line(
            &serde_json::to_string(&bundle).map_err(|e| e.to_string())?,
        );
        let body = redacted.into_bytes();
        std::fs::write(&path, body).map_err(|e| e.to_string())?;
        self.bundles.lock().await.insert(
            handle.clone(),
            BundleEntry {
                path,
                expires_at: Instant::now() + Duration::from_secs(300),
            },
        );
        Ok(json!({"handle": handle, "expires_in_seconds": 300}))
    }

    pub async fn save_bundle(&self, handle: &str, destination: &str) -> Result<Value, String> {
        if handle.len() > 64 || destination.len() > 4096 {
            return Err("invalid-request".into());
        }
        let entry = self
            .bundles
            .lock()
            .await
            .remove(handle)
            .ok_or_else(|| "bundle-expired-or-used".to_string())?;
        if entry.expires_at <= Instant::now() {
            let _ = std::fs::remove_file(entry.path);
            return Err("bundle-expired-or-used".into());
        }
        let path = entry.path;
        let destination = PathBuf::from(destination);
        if destination.as_os_str().is_empty() || !destination.is_absolute() || destination.is_dir()
        {
            let _ = std::fs::remove_file(path);
            return Err("invalid-destination".into());
        }
        std::fs::copy(&path, &destination).map_err(|e| {
            let _ = std::fs::remove_file(&path);
            e.to_string()
        })?;
        let _ = std::fs::remove_file(path);
        Ok(json!({"saved": true}))
    }

    pub async fn cancel_bundle(&self, handle: &str) -> Result<Value, String> {
        if let Some(entry) = self.bundles.lock().await.remove(handle) {
            let _ = std::fs::remove_file(entry.path);
        }
        Ok(json!({"cancelled": true}))
    }

    pub fn settings(&self) -> Result<Value, String> {
        Ok(engine_settings::read_settings(&self.data_dir))
    }

    pub fn set_settings(&self, timeout: u64) -> Result<Value, String> {
        engine_settings::update_settings(&self.data_dir, Some(timeout), None)
    }

    pub fn set_settings_patch(
        &self,
        timeout: Option<u64>,
        usage_tracker_enabled: Option<bool>,
    ) -> Result<Value, String> {
        engine_settings::update_settings(&self.data_dir, timeout, usage_tracker_enabled)
    }

    pub fn autostart(&self) -> Value {
        // Dev builds too: the manager-gpui settings toggle registers
        // `<exe> --start` — headless engine at sign-in, no UI window.
        let available = cfg!(windows) && std::env::var_os("KOSMOS_TEST_MODE").is_none();
        let enabled = available && windows_autostart_enabled();
        json!({"enabled": enabled, "available": available, "reason": if available { Value::Null } else { json!("unsupported-platform-or-test") }})
    }

    pub fn set_autostart(&self, enabled: bool) -> Result<Value, String> {
        if !self
            .autostart()
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("autostart-unavailable-in-dev-or-test".into());
        }
        set_windows_autostart(enabled)?;
        let state = self.autostart();
        if state.get("enabled").and_then(Value::as_bool) != Some(enabled) {
            return Err("autostart-readback-failed".into());
        }
        Ok(state)
    }
}

fn bounded_limit(value: Option<&Value>, max: usize) -> Result<usize, String> {
    let limit = value.and_then(Value::as_u64).unwrap_or(100);
    if limit == 0 || limit as usize > max {
        return Err("invalid-limit".into());
    }
    Ok(limit as usize)
}

fn parse_cursor(value: Option<&Value>) -> Result<usize, String> {
    let Some(value) = value else { return Ok(0) };
    let Some(cursor) = value.as_str() else {
        return Err("invalid-cursor".into());
    };
    cursor.parse::<usize>().map_err(|_| "invalid-cursor".into())
}

fn is_live_public_row(row: &Value) -> bool {
    row.get("deleted_at")
        .or_else(|| row.get("deletedAt"))
        .map(Value::is_null)
        .unwrap_or(true)
        && row
            .get("type_id")
            .or_else(|| row.get("typeId"))
            .and_then(Value::as_str)
            .is_none_or(|id| !id.is_empty() && !id.starts_with('_') && !id.starts_with("ark_"))
}

fn safe_row(row: &Value, query: Option<&str>) -> Option<Value> {
    let raw_type = row
        .get("type_id")
        .or_else(|| row.get("typeId"))
        .and_then(Value::as_str)?;
    let (type_id, type_version) = canonical_identity(raw_type)?;
    if row
        .get("type_version")
        .or_else(|| row.get("typeVersion"))
        .and_then(Value::as_str)
        .is_some_and(|version| version != type_version)
    {
        return None;
    }
    let id = row.get("id").and_then(Value::as_str)?;
    let title = row.get("title").and_then(Value::as_str).unwrap_or_default();
    let props = row
        .get("props_json")
        .or_else(|| row.get("propsJson"))
        .or_else(|| row.get("props"))
        .and_then(Value::as_object);
    let allowed = match type_id {
        "com.kosmos.note" => ["description"].as_slice(),
        "com.kosmos.task" => [
            "status",
            "priority",
            "scheduledAt",
            "dueAt",
            "reminderAt",
            "completedAt",
            "canceledAt",
            "recurrence",
            "checklist",
        ]
        .as_slice(),
        "com.kosmos.game" => [
            "playStatus",
            "userRating",
            "released",
            "description",
            "genres",
            "platforms",
        ]
        .as_slice(),
        _ => &[] as &[&str],
    };
    let mut fields = serde_json::Map::new();
    if let Some(props) = props {
        for key in allowed {
            if let Some(value) = props.get(*key) {
                fields.insert((*key).to_string(), value.clone());
            }
        }
    }
    let excerpt = query
        .and_then(|needle| {
            title
                .to_ascii_lowercase()
                .find(&needle.to_ascii_lowercase())
                .map(|_| title)
        })
        .unwrap_or(title);
    let links = row.get("links").and_then(Value::as_array).map(|links| {
        links.iter().filter_map(|link| {
            let target = link.get("target_object_id").or_else(|| link.get("targetObjectId")).and_then(Value::as_str)?;
            let link_type = link.get("link_type").or_else(|| link.get("linkType")).and_then(Value::as_str)?;
            Some(json!({"id": link.get("id").and_then(Value::as_str).unwrap_or_default(), "target_object_id": target, "link_type": link_type}))
        }).collect::<Vec<_>>()
    }).unwrap_or_default();
    Some(json!({
        "id": id,
        "type_id": type_id,
        "type_version": type_version,
        "title": crate::observability::redact_text(title),
        "fields": fields,
        "created_at": row.get("created_at").or_else(|| row.get("createdAt")).and_then(Value::as_str),
        "updated_at": row.get("updated_at").or_else(|| row.get("updatedAt")).and_then(Value::as_str),
        "excerpt": crate::observability::redact_text(excerpt).chars().take(256).collect::<String>(),
        "links": links,
    }))
}

fn managed_bytes(root: &Path, package_store: &Path) -> u64 {
    ["ark.db", "ark.db-wal", "ark.db-shm"]
        .iter()
        .map(|name| file_bytes(&root.join(name)))
        .sum::<u64>()
        .saturating_add(directory_bytes(package_store))
}
fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}
fn directory_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let p = entry.path();
            if p.is_dir() {
                directory_bytes(&p)
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

#[cfg(windows)]
fn windows_registry_command() -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut command = std::process::Command::new("reg.exe");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(windows)]
fn windows_autostart_enabled() -> bool {
    windows_registry_command()
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "Kosmos Engine",
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn windows_autostart_enabled() -> bool {
    false
}

#[cfg(windows)]
fn set_windows_autostart(enabled: bool) -> Result<(), String> {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let mut command = windows_registry_command();
    if enabled {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let startup_command = format!("\"{}\" --start", executable.display());
        command
            .args(["add", key, "/v", "Kosmos Engine", "/t", "REG_SZ", "/d"])
            .arg(startup_command)
            .args(["/f"]);
    } else {
        command.args(["delete", key, "/v", "Kosmos Engine", "/f"]);
    }
    let result = command.output();
    result.map_err(|e| e.to_string()).and_then(|output| {
        if output.status.success() || !enabled {
            Ok(())
        } else {
            Err("autostart-registration-failed".into())
        }
    })
}

#[cfg(not(windows))]
fn set_windows_autostart(_enabled: bool) -> Result<(), String> {
    Err("autostart-unavailable-in-dev-or-test".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn autostart_registry_commands_share_the_no_console_guard() {
        let production = include_str!("manager_api.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert_eq!(production.matches("windows_registry_command()").count(), 3);
        assert_eq!(production.matches("Command::new(\"reg.exe\")").count(), 1);
        assert!(production.contains("command.creation_flags(CREATE_NO_WINDOW)"));
    }

    #[test]
    fn settings_are_two_state_and_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let s = ManagerState::new(dir.path().to_path_buf());
        assert_eq!(
            s.settings().unwrap()["desktop_host"]["warm_timeout_seconds"],
            300
        );
        assert!(s.set_settings(1).is_err());
        assert_eq!(
            s.set_settings(0).unwrap()["desktop_host"]["warm_timeout_seconds"],
            0
        );
    }

    #[test]
    fn managed_bytes_counts_ark_sidecars_and_package_store_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ark.db"), [0_u8; 4]).unwrap();
        std::fs::write(dir.path().join("ark.db-wal"), [0_u8; 3]).unwrap();
        std::fs::write(dir.path().join("ark.db-shm"), [0_u8; 2]).unwrap();
        std::fs::write(dir.path().join("unmanaged.log"), [0_u8; 99]).unwrap();
        let packages = dir.path().join("packages");
        std::fs::create_dir_all(&packages).unwrap();
        std::fs::write(packages.join("state.json"), [0_u8; 5]).unwrap();
        assert_eq!(managed_bytes(dir.path(), &packages), 14);
    }

    #[test]
    fn canonical_type_sources_deduplicate_aliases_and_prefer_canonical_ids() {
        let sources = canonical_type_sources(&[
            json!({"id": "note_obj"}),
            json!({"id": "com.kosmos.note"}),
            json!({"id": "task_obj"}),
            json!({"id": "com.kosmos.game"}),
            json!({"id": "unknown"}),
        ]);
        assert_eq!(
            sources,
            vec![
                ("com.kosmos.note", "1.0.0", "com.kosmos.note".to_owned()),
                ("com.kosmos.task", "1.0.0", "task_obj".to_owned()),
                ("com.kosmos.game", "1.0.0", "com.kosmos.game".to_owned()),
            ]
        );
    }

    #[test]
    fn manager_rows_are_bounded_redacted_and_body_free() {
        let row = json!({
            "id": "n1",
            "type_id": "note_obj",
            "title": r"C:\Users\alice\private note",
            "content_json": {"secret": "body"},
            "props_json": {"token": "abc"},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "deleted_at": Value::Null,
        });
        let safe = safe_row(&row, None).expect("canonical projection");
        assert!(safe.get("content_json").is_none());
        assert!(safe.get("props_json").is_none());
        assert!(!safe.to_string().contains("alice"));
        assert!(safe.get("excerpt").is_some());
        assert_eq!(bounded_limit(Some(&json!(200)), MAX_BROWSE_PAGE), Ok(200));
        assert!(bounded_limit(Some(&json!(201)), MAX_BROWSE_PAGE).is_err());
        assert!(parse_cursor(Some(&json!("nope"))).is_err());

        let camel = json!({
            "id": "n2",
            "typeId": "note_obj",
            "title": "Заметка",
            "createdAt": "2026-02-01T00:00:00Z",
            "updatedAt": "2026-02-02T00:00:00Z",
            "deletedAt": Value::Null,
        });
        assert!(is_live_public_row(&camel));
        let camel_safe = safe_row(&camel, None).expect("legacy boundary projection");
        assert_eq!(camel_safe["type_id"], "com.kosmos.note");
        assert_eq!(camel_safe["type_version"], "1.0.0");
        assert_eq!(camel_safe["created_at"], "2026-02-01T00:00:00Z");

        let task = json!({
            "id": "task-1", "type_id": "task_obj", "type_version": "0.0.0-legacy",
            "title": "Task", "props_json": {"status": "done", "api_token": "secret", "local_path": "/private"}
        });
        assert!(safe_row(&task, None).is_none());
        let game = json!({
            "id": "game-1", "type_id": "game_obj", "title": "Game",
            "props_json": {"playStatus": "completed", "genres": ["rpg"], "secret": "drop"},
            "links": [{"id": "l1", "targetObjectId": "note-1", "linkType": "note"}]
        });
        let game_safe = safe_row(&game, None).expect("game projection");
        assert_eq!(game_safe["type_id"], "com.kosmos.game");
        assert_eq!(game_safe["fields"]["playStatus"], "completed");
        assert!(game_safe["fields"].get("secret").is_none());
        assert_eq!(game_safe["links"][0]["target_object_id"], "note-1");

        let tombstone = json!({"typeId": "note_obj", "deletedAt": "2026-02-03T00:00:00Z"});
        assert!(!is_live_public_row(&tombstone));
    }
    #[tokio::test]
    async fn bundle_handle_is_one_time_and_cancel_removes() {
        let dir = tempfile::tempdir().unwrap();
        let s = ManagerState::new(dir.path().to_path_buf());
        let h = s
            .create_bundle(json!({"secret":"[REDACTED]"}), json!({}))
            .await
            .unwrap()["handle"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(s
            .save_bundle(&h, dir.path().join("x.json").to_str().unwrap())
            .await
            .is_ok());
        assert!(s
            .save_bundle(&h, dir.path().join("y.json").to_str().unwrap())
            .await
            .is_err());
        let h2 = s.create_bundle(json!({}), json!({})).await.unwrap()["handle"]
            .as_str()
            .unwrap()
            .to_string();
        s.cancel_bundle(&h2).await.unwrap();
        assert!(!dir
            .path()
            .join("support-bundles")
            .join(format!("{h2}.json"))
            .exists());
    }

    #[tokio::test]
    async fn support_bundle_uses_shared_redaction_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let state = ManagerState::new(dir.path().to_path_buf());
        let handle = state
            .create_bundle(
                json!({
                    "token": "secret-token",
                    "body": {"private": "object body"},
                    "package_path": r"C:\Users\alice\packages\demo.kspkg",
                    "payload": "raw payload",
                }),
                json!({"authorization": "Bearer abc"}),
            )
            .await
            .unwrap()["handle"]
            .as_str()
            .unwrap()
            .to_owned();
        let destination = dir.path().join("bundle.json");
        state
            .save_bundle(&handle, destination.to_str().unwrap())
            .await
            .unwrap();
        let output = std::fs::read_to_string(destination).unwrap();
        for secret in [
            "secret-token",
            "object body",
            "raw payload",
            "alice",
            "Bearer abc",
        ] {
            assert!(!output.contains(secret), "bundle leaked {secret}: {output}");
        }
        assert!(output.contains("[REDACTED]"));
    }
}
