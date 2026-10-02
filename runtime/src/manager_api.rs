use crate::diagnostics::SharedRpcDiagnostics;
use crate::engine_settings;
use crate::package_service::PackageService;
use crate::protocol_usage::ProtocolUsageStore;
use crate::updater::UpdaterService;
use crate::usage_tracker::UsageTrackerDiagnosticsState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

mod data;
mod quarantine;
mod storage;

const MAX_LOG_TAIL: usize = 200;

#[derive(Clone)]
pub struct ManagerState {
    data_dir: Arc<PathBuf>,
    bundles: Arc<Mutex<HashMap<String, BundleEntry>>>,
    updater: Arc<UpdaterService>,
}

struct BundleEntry {
    path: PathBuf,
    expires_at: Instant,
}

impl ManagerState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            updater: UpdaterService::new(data_dir.clone()),
            data_dir: Arc::new(data_dir),
            bundles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn updater(&self) -> Arc<UpdaterService> {
        self.updater.clone()
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
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
        let available = cfg!(windows) && std::env::var_os("MUNDUS_TEST_MODE").is_none();
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

// pub(crate): the installer's `post-install` subcommand reuses the
// StartupApproved marker layout and the Run/Approved subkey constants
// (KOS-306).
#[cfg(windows)]
pub(crate) mod windows_autostart;

#[cfg(windows)]
fn windows_autostart_enabled() -> bool {
    windows_autostart::enabled()
}

#[cfg(not(windows))]
fn windows_autostart_enabled() -> bool {
    false
}

#[cfg(windows)]
fn set_windows_autostart(enabled: bool) -> Result<(), String> {
    windows_autostart::set(enabled)
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
    fn autostart_marker_matches_the_os_layout() {
        // StartupApproved\Run marker: state byte, three zero bytes, FILETIME.
        let disabled = windows_autostart::approved_marker(true);
        assert_eq!(disabled.len(), 12);
        assert_eq!(disabled[0], 3);
        assert_eq!(&disabled[1..4], &[0, 0, 0]);
        assert!(u64::from_le_bytes(disabled[4..].try_into().unwrap()) > 0);
        assert_eq!(windows_autostart::approved_marker(false)[0], 2);
    }

    #[cfg(windows)]
    #[test]
    fn autostart_uses_the_registry_api_not_a_child_process() {
        // A spawned reg.exe needed CREATE_NO_WINDOW babysitting and text
        // parsing; the Win32 calls have neither surface.
        let production = include_str!("manager_api.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let autostart = include_str!("manager_api/windows_autostart.rs");
        assert!(!production.contains("Command::new"));
        assert!(!autostart.contains("Command::new"));
        assert!(autostart.contains("RegSetKeyValueW"));
        // The subkey constants live in installer::registry, shared with the
        // post-install subcommand (KOS-306).
        let registry = include_str!("installer/registry.rs");
        assert!(registry.contains("Explorer\\StartupApproved\\Run"));
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
