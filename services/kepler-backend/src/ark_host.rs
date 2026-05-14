// Управление child-процессом ark-core-rpc.
//
// AC1 (часть): через WS-dispatcher переслать ARK-операцию в child, получить ответ.
//
// Wire-протокол ark-core-rpc (см. packages/ark-core/rust/src/main.rs):
//   Request:  {"operation": "<name>", "_req_id": "<id>", ...params}
//   Response: {"_req_id": "<id>", "ok": true|false, "data"?, "error"?}
//   Event:    {"event": "<name>", ...}     (нет _req_id, нет ok)
//
// Дизайн:
//   - ArkHost держит child + tokio writer task + reader task.
//   - request(op, params) — выдаёт req_id, кладёт oneshot::Sender в pending map,
//     сериализует JSON, отправляет через mpsc в writer task. Возвращает Future.
//   - Reader task парсит stdout по строкам, по `_req_id` resolve'ит pending, по
//     `event` — броадкаст в watch/broadcast channel.
//   - Unit tests используют mock через trait-обобщение по AsyncRead/AsyncWrite,
//     чтобы не требовать собранного ark-core-rpc.exe.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

const ARK_HOST_INTERNAL_REQ_ID_PREFIX: &str = "kepler-host-";

#[derive(Debug, Error)]
pub enum ArkHostError {
    #[error("ark-core-rpc binary not found in candidates: {0:?}")]
    BinaryNotFound(Vec<PathBuf>),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ark-core-rpc returned error: {0}")]
    RpcError(String),
    #[error("ark-core-rpc child died unexpectedly")]
    ChildDead,
    #[error("request channel closed")]
    ChannelClosed,
    #[error("response missing _req_id")]
    UncorrelatedResponse,
}

pub type ArkResult<T> = Result<T, ArkHostError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArkResponse {
    pub ok: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ArkFrame {
    Response {
        req_id: String,
        response: ArkResponse,
    },
    Event {
        event: String,
        payload: serde_json::Value,
    },
}

/// Resolve путь к ark-core-rpc бинарю. Порядок:
///   1. ENV `ARK_CORE_RPC_PATH`
///   2. рядом с current_exe (production bundling)
///   3. dev fallback: ../../packages/ark-core/rust/target/{release,debug}/ark-core-rpc[.exe]
pub fn resolve_ark_core_rpc_path() -> ArkResult<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(p) = std::env::var("ARK_CORE_RPC_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
        candidates.push(path);
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            #[cfg(windows)]
            let candidate = parent.join("ark-core-rpc.exe");
            #[cfg(not(windows))]
            let candidate = parent.join("ark-core-rpc");
            if candidate.exists() {
                return Ok(candidate);
            }
            candidates.push(candidate);
        }
    }

    let dev_relative = [
        "../../target/release/ark-core-rpc.exe",
        "../../target/release/ark-core-rpc",
        "../../target/debug/ark-core-rpc.exe",
        "../../target/debug/ark-core-rpc",
    ];
    for rel in dev_relative {
        let p = PathBuf::from(rel);
        if p.exists() {
            return Ok(p);
        }
        candidates.push(p);
    }

    Err(ArkHostError::BinaryNotFound(candidates))
}

/// Сообщение от запроса к writer task.
struct OutgoingRequest {
    payload: Vec<u8>, // JSON line с '\n'
}

/// ArkHost — управление child-процессом ark-core-rpc через stdio.
pub struct ArkHost {
    next_req_id: AtomicU64,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>>,
    writer_tx: mpsc::UnboundedSender<OutgoingRequest>,
    events_tx: broadcast::Sender<(String, serde_json::Value)>,
    _child: tokio::process::Child,
}

impl ArkHost {
    /// Спавн ark-core-rpc как child, отправка `init` с db_path, ожидание ok.
    pub async fn spawn(binary_path: &Path, db_path: &str) -> ArkResult<Self> {
        use tokio::process::Command;

        let mut child = Command::new(binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ArkHostError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "child stdin missing",
            ))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ArkHostError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "child stdout missing",
            ))
        })?;

        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, _) = broadcast::channel::<(String, serde_json::Value)>(128);
        let (writer_tx, writer_rx) = mpsc::unbounded_channel::<OutgoingRequest>();

        // Writer task: drains mpsc → child.stdin.
        let writer_task_stdin = stdin;
        tokio::spawn(writer_loop(writer_task_stdin, writer_rx));

        // Reader task: parses stdout lines.
        let pending_for_reader = pending.clone();
        let events_for_reader = events_tx.clone();
        tokio::spawn(reader_loop(stdout, pending_for_reader, events_for_reader));

        let host = ArkHost {
            next_req_id: AtomicU64::new(0),
            pending,
            writer_tx,
            events_tx,
            _child: child,
        };

        // Init.
        let init_response = host
            .request(
                "init",
                serde_json::json!({
                    "dbPath": db_path,
                }),
            )
            .await?;
        if !init_response.ok {
            return Err(ArkHostError::RpcError(
                init_response.error.unwrap_or_default(),
            ));
        }
        Ok(host)
    }

    pub async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> ArkResult<ArkResponse> {
        let req_id = format!(
            "{}{}",
            ARK_HOST_INTERNAL_REQ_ID_PREFIX,
            self.next_req_id.fetch_add(1, Ordering::Relaxed)
        );

        let mut envelope = match params {
            serde_json::Value::Object(map) => map,
            serde_json::Value::Null => serde_json::Map::new(),
            other => {
                let mut m = serde_json::Map::new();
                m.insert("params".to_string(), other);
                m
            }
        };
        envelope.insert(
            "operation".to_string(),
            serde_json::Value::String(operation.to_string()),
        );
        envelope.insert(
            "_req_id".to_string(),
            serde_json::Value::String(req_id.clone()),
        );

        let mut line = serde_json::to_vec(&serde_json::Value::Object(envelope))?;
        line.push(b'\n');

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(req_id, tx);
        }

        self.writer_tx
            .send(OutgoingRequest { payload: line })
            .map_err(|_| ArkHostError::ChannelClosed)?;

        rx.await.map_err(|_| ArkHostError::ChildDead)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, serde_json::Value)> {
        self.events_tx.subscribe()
    }
}

async fn writer_loop(
    mut stdin: tokio::process::ChildStdin,
    mut rx: mpsc::UnboundedReceiver<OutgoingRequest>,
) {
    while let Some(req) = rx.recv().await {
        if stdin.write_all(&req.payload).await.is_err() {
            break;
        }
        if stdin.flush().await.is_err() {
            break;
        }
    }
}

async fn reader_loop(
    stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>>,
    events_tx: broadcast::Sender<(String, serde_json::Value)>,
) {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(value) => dispatch_frame(value, &pending, &events_tx).await,
            Err(_) => {
                // Малформированная строка — лог в Phase 2, сейчас игнор.
            }
        }
    }
}

async fn dispatch_frame(
    value: serde_json::Value,
    pending: &Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>>,
    events_tx: &broadcast::Sender<(String, serde_json::Value)>,
) {
    // Событие отличается наличием "event" поля.
    if let Some(event_name) = value.get("event").and_then(|v| v.as_str()) {
        let event_name = event_name.to_string();
        let _ = events_tx.send((event_name, value));
        return;
    }

    let req_id = match value.get("_req_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return, // нет _req_id и нет event — игнор.
    };

    let response = ArkResponse {
        ok: value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        data: value.get("data").cloned().unwrap_or(serde_json::Value::Null),
        error: value
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    };

    let mut map = pending.lock().await;
    if let Some(sender) = map.remove(&req_id) {
        let _ = sender.send(response);
    }
}

// ----- Unit tests: dispatcher logic без real child -----
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dispatch_response_to_pending() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, _) = broadcast::channel(8);

        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert("req-1".to_string(), tx);

        let frame = serde_json::json!({
            "_req_id": "req-1",
            "ok": true,
            "data": { "result": "foo" }
        });

        dispatch_frame(frame, &pending, &events_tx).await;
        let response = rx.await.expect("response delivered");
        assert!(response.ok);
        assert_eq!(response.data["result"], "foo");
    }

    #[tokio::test]
    async fn dispatch_response_with_error() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, _) = broadcast::channel(8);

        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert("req-2".to_string(), tx);

        let frame = serde_json::json!({
            "_req_id": "req-2",
            "ok": false,
            "error": "something failed"
        });

        dispatch_frame(frame, &pending, &events_tx).await;
        let response = rx.await.unwrap();
        assert!(!response.ok);
        assert_eq!(response.error.as_deref(), Some("something failed"));
    }

    #[tokio::test]
    async fn dispatch_event_to_broadcast() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, mut events_rx) = broadcast::channel(8);

        let frame = serde_json::json!({
            "event": "entity_changed",
            "entity_type": "object",
            "id": "abc"
        });

        dispatch_frame(frame, &pending, &events_tx).await;
        let (event_name, payload) = events_rx.recv().await.unwrap();
        assert_eq!(event_name, "entity_changed");
        assert_eq!(payload["entity_type"], "object");
    }

    #[tokio::test]
    async fn dispatch_uncorrelated_response_is_dropped() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, _) = broadcast::channel(8);

        let frame = serde_json::json!({
            "_req_id": "unknown",
            "ok": true,
            "data": {}
        });

        dispatch_frame(frame, &pending, &events_tx).await;
        assert!(pending.lock().await.is_empty());
    }

    #[tokio::test]
    async fn dispatch_frame_without_req_id_or_event_is_ignored() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, mut events_rx) = broadcast::channel(8);

        let frame = serde_json::json!({ "ok": true, "data": {} });
        dispatch_frame(frame, &pending, &events_tx).await;

        // pending пуст, events пуст
        assert!(pending.lock().await.is_empty());
        assert!(events_rx.try_recv().is_err());
    }

    #[test]
    fn resolve_returns_error_when_no_candidates_exist() {
        // В чистой среде без env и без installed бинаря — ожидаем BinaryNotFound.
        // На dev-машине может найти dev binary, поэтому проверяем условно.
        std::env::remove_var("ARK_CORE_RPC_PATH");
        match resolve_ark_core_rpc_path() {
            Ok(path) => {
                // Если что-то нашлось — это валидный путь к существующему файлу.
                assert!(path.exists(), "resolved path doesn't exist: {path:?}");
            }
            Err(ArkHostError::BinaryNotFound(candidates)) => {
                assert!(!candidates.is_empty(), "should report attempted paths");
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
}
