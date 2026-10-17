// In-process ARK host — wraps `ark_core::service::ArkService` directly.
// (Was: supervisor for the ark-core-rpc child process over newline-delimited
// JSON-RPC. The sidecar is gone; the Engine hosts ARK as a library now.)

use ark_core::canonical_types::game::{GameMutationResult, GameRecord, GameUpsertCommand};
use ark_core::service::ArkService;
use ark_core::type_registry::TypeRegistration;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Debug, Error)]
pub enum ArkHostError {
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ark service returned error: {0}")]
    RpcError(String),
    #[error("ark service unavailable: {0}")]
    Unavailable(String),
}

pub type ArkResult<T> = Result<T, ArkHostError>;

impl From<ark_core::service::ArkServiceError> for ArkHostError {
    fn from(error: ark_core::service::ArkServiceError) -> Self {
        match error {
            ark_core::service::ArkServiceError::Request(message) => Self::RpcError(message),
            ark_core::service::ArkServiceError::Busy(message) => {
                Self::RpcError(format!("ark service busy: {message}"))
            }
            other => Self::Unavailable(other.to_string()),
        }
    }
}

/// Same wire shape the sidecar returned; kept so the ~36 runtime callers and
/// the `{"ok","data","error"}` contract stay unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkResponse {
    pub ok: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

/// In-process ARK service handle. Requests are dispatched to the service's
/// dedicated worker thread (sequential, FIFO — same ordering the sidecar's
/// serial stdin loop provided, so single-writer/transaction semantics are
/// unchanged). Events arrive from the service's broadcast bus and are
/// re-published on `events_tx` together with engine-side `emit_event`s.
pub struct ArkHost {
    service: Option<ArkService>,
    events_tx: broadcast::Sender<(String, serde_json::Value)>,
    events_task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for ArkHost {
    fn drop(&mut self) {
        if let Some(events_task) = self.events_task.take() {
            events_task.abort();
        }
    }
}

impl ArkHost {
    /// Trusted package-install seam. The ARK side owns the SQLite transaction;
    /// callers never receive a database handle or issue arbitrary SQL.
    pub async fn register_package_definitions(
        &self,
        registrations: Vec<TypeRegistration>,
    ) -> ArkResult<()> {
        let response = self
            .request(
                "types.registerPackageDefinitions",
                serde_json::json!({ "registrations": registrations }),
            )
            .await?;
        typed_response(response, "package_definition_registration_failed")
    }
    pub async fn canonical_game_list(&self) -> ArkResult<Vec<GameRecord>> {
        let response = self
            .request(
                "canonical.game.list",
                serde_json::json!({ "deviceId": stable_device_id() }),
            )
            .await?;
        typed_response(response, "game_list_failed")
    }

    pub async fn canonical_game_get(&self, id: &str) -> ArkResult<Option<GameRecord>> {
        let response = self
            .request(
                "canonical.game.get",
                serde_json::json!({ "id": id, "deviceId": stable_device_id() }),
            )
            .await?;
        typed_response(response, "game_not_found")
    }

    pub async fn canonical_game_upsert(
        &self,
        mut game: GameUpsertCommand,
    ) -> ArkResult<GameMutationResult> {
        game.device_id = Some(stable_device_id());
        let response = self
            .request("canonical.game.upsert", serde_json::json!({ "game": game }))
            .await?;
        typed_response(response, "game_upsert_failed")
    }

    /// Open (create if needed) the ARK database at `db_path` and start the
    /// service worker. Same semantics as the old `ArkHost::spawn` + `init`.
    pub async fn open(db_path: &str) -> ArkResult<Self> {
        // Host config (KOS-287): the real %SystemRoot%, resolved once here via
        // GetSystemWindowsDirectoryW. ark-core marks exes under it as system
        // apps; a request param would let clients lie about that, so it lives
        // in construction-time config. On failure we warn and pass None —
        // nothing gets misclassified.
        let service = ArkService::open_with_host_config(
            db_path,
            ark_core::service::ArkHostConfig {
                windows_dir: system_windows_dir().map(str::to_string),
            },
        )
        .await
        .map_err(|error| {
            ArkHostError::RpcError(format!(
                "failed to open ARK database at {db_path} (locked by a stale ark-core-rpc \
                 or another Engine?): {error}"
            ))
        })?;
        let (events_tx, _) = broadcast::channel(128);
        let mut service_events = service.subscribe();
        let forwarder_tx = events_tx.clone();
        let events_task = tokio::spawn(async move {
            loop {
                match service_events.recv().await {
                    Ok(value) => {
                        if let Some(name) = value.get("event").and_then(serde_json::Value::as_str) {
                            let _ = forwarder_tx.send((name.to_owned(), value));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(Self {
            service: Some(service),
            events_tx,
            events_task: Some(events_task),
        })
    }

    /// Execute one ARK request — same `{"operation": "...", ...params}`
    /// envelope and `{"ok","data","error"}` response shape as the sidecar had.
    /// `ok:false` errors stay *responses* (callers inspect them); only a dead
    /// service maps to `Err(ArkHostError::Unavailable)`.
    pub async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> ArkResult<ArkResponse> {
        let service = self
            .service
            .as_ref()
            .expect("ArkHost service is present until shutdown");
        match service.call(operation, params).await {
            Ok(data) => Ok(ArkResponse {
                ok: true,
                data,
                error: None,
            }),
            Err(ark_core::service::ArkServiceError::Request(error)) => Ok(ArkResponse {
                ok: false,
                data: serde_json::Value::Null,
                error: Some(error),
            }),
            Err(ark_core::service::ArkServiceError::Busy(error)) => Ok(ArkResponse {
                ok: false,
                data: serde_json::Value::Null,
                error: Some(format!("ark service busy: {error}")),
            }),
            Err(error) => Err(ArkHostError::Unavailable(error.to_string())),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, serde_json::Value)> {
        self.events_tx.subscribe()
    }
    /// Публикует engine-side событие в общий broadcast — все подключённые WS
    /// клиенты получат его как обычный ark event. Для событий, которые
    /// порождает сам runtime (например `db_restored` после `db_backup_restore`:
    /// Core завершает restore синхронным ответом и сам события не шлёт).
    pub fn emit_event(&self, payload: serde_json::Value) {
        if let Some(name) = payload.get("event").and_then(serde_json::Value::as_str) {
            let _ = self.events_tx.send((name.to_owned(), payload));
        }
    }
    /// Diagnostics: number of ARK handler panics the service worker caught
    /// (each followed by a DB reopen).
    pub fn panic_count(&self) -> u64 {
        self.service
            .as_ref()
            .expect("ArkHost service is present until shutdown")
            .panic_count()
    }

    /// Stop accepting work and wait for the ARK worker to close its database
    /// owner before returning. Call this during orderly Engine shutdown and
    /// before removing a database directory in tests.
    pub async fn shutdown(mut self, deadline: std::time::Instant) -> ArkResult<()> {
        if let Some(events_task) = self.events_task.take() {
            events_task.abort();
            let _ = events_task.await;
        }
        let service = self
            .service
            .take()
            .expect("ArkHost service is present until shutdown");
        service.shutdown(deadline).await.map_err(Into::into)
    }
}

fn stable_device_id() -> String {
    crate::brand::env("DEVICE_ID")
        .filter(|id| !id.trim().is_empty())
        .map(|id| id.trim().to_owned())
        .unwrap_or_else(|| "ark-host-local".to_owned())
}

fn typed_response<T: for<'de> Deserialize<'de>>(
    response: ArkResponse,
    fallback: &str,
) -> ArkResult<T> {
    if !response.ok {
        return Err(ArkHostError::RpcError(
            response.error.unwrap_or_else(|| fallback.into()),
        ));
    }
    serde_json::from_value(response.data).map_err(ArkHostError::Json)
}

/// Real %SystemRoot% via `GetSystemWindowsDirectoryW`, memoized. `None`
/// (resolution failed, or non-Windows host) means usage analytics marks
/// nothing as system — never a hard-coded `c:\windows` guess (KOS-287).
#[cfg(target_os = "windows")]
fn system_windows_dir() -> Option<&'static str> {
    use std::sync::OnceLock;
    static DIR: OnceLock<Option<String>> = OnceLock::new();
    DIR.get_or_init(|| {
        use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
        let mut buf = [0u16; 260];
        let len = unsafe { GetSystemWindowsDirectoryW(Some(&mut buf)) };
        if len == 0 || len as usize >= buf.len() {
            tracing::warn!(
                "GetSystemWindowsDirectoryW failed — usage analytics will not flag system apps"
            );
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    })
    .as_deref()
}

#[cfg(not(target_os = "windows"))]
fn system_windows_dir() -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn shutdown(host: ArkHost) {
        host.shutdown(std::time::Instant::now() + std::time::Duration::from_secs(30))
            .await
            .unwrap();
    }

    /// Envelope coverage moved here from the sidecar tests: an ok:true
    /// response carries data, an ok:false carries the handler error, and an
    /// unavailable service maps to `Unavailable`.
    #[tokio::test]
    async fn request_returns_ok_data_and_error_envelopes() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("ark.db");
        let host = ArkHost::open(db_path.to_str().unwrap()).await.unwrap();

        let ok = host
            .request("load_all", serde_json::json!({}))
            .await
            .unwrap();
        assert!(ok.ok);
        assert!(ok.error.is_none());
        assert!(ok.data.is_object());

        let failed = host
            .request("get_object", serde_json::json!({"id": "missing"}))
            .await
            .unwrap();
        // unknown/missing object returns null data or an error envelope —
        // either way it must be a well-formed ArkResponse, not an Err.
        assert!(failed.ok || failed.error.is_some());

        let malformed = host
            .request("no.such.operation", serde_json::json!({}))
            .await
            .unwrap();
        assert!(!malformed.ok);
        assert!(malformed.error.is_some());
        shutdown(host).await;
    }

    #[tokio::test]
    async fn saturated_request_uses_the_existing_error_envelope() {
        let dir = tempfile::tempdir().unwrap();
        let host = ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap();

        let response = host
            .request(
                "test.oversized",
                serde_json::json!({ "payload": "x".repeat(9 * 1024 * 1024) }),
            )
            .await
            .expect("admission errors remain ArkResponse values");
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("admission size limits"));
        shutdown(host).await;
    }

    #[tokio::test]
    async fn request_response_bytes_match_ark_api_v1_fixtures() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            operation: String,
            params: serde_json::Value,
            response: String,
        }

        let dir = tempfile::tempdir().unwrap();
        let host = ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap();
        let fixtures = include_str!("../tests/fixtures/ark-api-v1/responses.jsonl");
        let mut count = 0;
        for line in fixtures.lines() {
            let fixture: Fixture = serde_json::from_str(line).unwrap();
            let response = host
                .request(&fixture.operation, fixture.params)
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_string(&response).unwrap(),
                fixture.response,
                "response bytes changed for {}",
                fixture.operation
            );
            count += 1;
        }
        assert_eq!(count, 2, "ArkResponse golden corpus is incomplete");
        shutdown(host).await;
    }

    #[tokio::test]
    async fn emit_event_reaches_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let host = ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap();
        let mut rx = host.subscribe_events();
        host.emit_event(serde_json::json!({"event": "ark_host_test_event"}));
        let (name, _) = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(name, "ark_host_test_event");
        shutdown(host).await;
    }

    /// KOS-287: the real %SystemRoot% is resolved at ArkHost::open and fed to
    /// usage analytics as host config — no request param, no env guess.
    #[tokio::test]
    async fn analytics_marks_system_app_from_host_windows_dir() {
        let Some(windows_dir) = system_windows_dir() else {
            return; // non-Windows host: nothing to classify
        };
        let dir = tempfile::tempdir().unwrap();
        let host = ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap();
        let exe = format!("{windows_dir}\\explorer.exe");
        host.request(
            "upsert_tracked_app",
            serde_json::json!({
                "tracked_app": {
                    "id": "app-explorer",
                    "platform": "windows",
                    "exePath": exe,
                    "normalizedExePath": exe.to_lowercase(),
                    "processName": "explorer.exe",
                    "displayName": "File Explorer",
                    "publisher": null,
                    "iconRef": null,
                    "firstSeenAt": "2026-01-01T00:00:00.000Z",
                    "lastSeenAt": "2026-01-01T00:10:00.000Z",
                }
            }),
        )
        .await
        .unwrap();
        host.request(
            "upsert_usage_session",
            serde_json::json!({
                "usage_session": {
                    "id": "s-1",
                    "trackedAppId": "app-explorer",
                    "deviceId": "d",
                    "deviceName": "D",
                    "platform": "windows",
                    "startedAt": "2026-01-01T00:00:00.000Z",
                    "endedAt": "2026-01-01T00:10:00.000Z",
                    "runtimeMs": 600_000,
                    "foregroundMs": 600_000,
                    "idleMs": 0,
                    "windowTitle": null,
                    "processName": "explorer.exe",
                    "exePath": exe,
                    "pidStart": null,
                    "pidEnd": null,
                }
            }),
        )
        .await
        .unwrap();

        let report = host
            .request("get_usage_analytics", serde_json::json!({}))
            .await
            .unwrap();
        let explorer = report.data["topApps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["processName"] == "explorer.exe")
            .expect("explorer row present");
        assert_eq!(explorer["isSystem"], true);
    }
}
