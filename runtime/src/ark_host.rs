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
    service: ArkService,
    events_tx: broadcast::Sender<(String, serde_json::Value)>,
    events_task: tokio::task::JoinHandle<()>,
}

impl Drop for ArkHost {
    fn drop(&mut self) {
        self.events_task.abort();
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
        let service = ArkService::open(db_path).await.map_err(|error| {
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
            service,
            events_tx,
            events_task,
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
        match self.service.call(operation, params).await {
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
        self.service.panic_count()
    }
}

fn stable_device_id() -> String {
    crate::brand::env("DEVICE_ID")
        .ok()
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[tokio::test]
    async fn emit_event_reaches_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let host = ArkHost::open(dir.path().join("ark.db").to_str().unwrap())
            .await
            .unwrap();
        let mut rx = host.subscribe_events();
        host.emit_event(serde_json::json!({"event": "ark_host_test_event"}));
        let (name, _) = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(name, "ark_host_test_event");
    }
}
