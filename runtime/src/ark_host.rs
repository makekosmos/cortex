// Управление child-процессом ark-core-rpc через newline-delimited JSON-RPC.

use ark_core::canonical_types::game::{GameMutationResult, GameRecord, GameUpsertCommand};
use ark_core::type_registry::TypeRegistration;
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

pub fn resolve_ark_core_rpc_path() -> ArkResult<PathBuf> {
    let mut candidates = Vec::new();
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
            let packaged = parent.join("Kosmos Data Engine.exe");
            #[cfg(not(windows))]
            let packaged = parent.join("Kosmos Data Engine");
            if packaged.exists() {
                return Ok(packaged);
            }
            candidates.push(packaged);
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
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let backend_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                manifest_dir.join(path)
            }
        })
        .unwrap_or_else(|| manifest_dir.join("../target"));
    #[cfg(debug_assertions)]
    let workspace_candidates = [
        backend_target.join("debug/ark-core-rpc.exe"),
        backend_target.join("debug/ark-core-rpc"),
        backend_target.join("release/ark-core-rpc.exe"),
        backend_target.join("release/ark-core-rpc"),
    ];
    #[cfg(not(debug_assertions))]
    let workspace_candidates = [
        backend_target.join("release/ark-core-rpc.exe"),
        backend_target.join("release/ark-core-rpc"),
        backend_target.join("debug/ark-core-rpc.exe"),
        backend_target.join("debug/ark-core-rpc"),
    ];
    for path in workspace_candidates {
        if path.exists() {
            return Ok(path);
        }
        candidates.push(path);
    }
    Err(ArkHostError::BinaryNotFound(candidates))
}

struct OutgoingRequest {
    payload: Vec<u8>,
}

pub struct ArkHost {
    next_req_id: AtomicU64,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>>,
    writer_tx: mpsc::UnboundedSender<OutgoingRequest>,
    events_tx: broadcast::Sender<(String, serde_json::Value)>,
    stderr_tail: Arc<std::sync::Mutex<crate::observability::BoundedTextTail>>,
    _child: tokio::process::Child,
}

impl Drop for ArkHost {
    fn drop(&mut self) {
        let _ = self._child.start_kill();
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

    pub async fn spawn(binary_path: &Path, db_path: &str) -> ArkResult<Self> {
        use tokio::process::Command;
        let mut command = Command::new(binary_path);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ArkHostError::Io(std::io::Error::other("child stdin missing")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ArkHostError::Io(std::io::Error::other("child stdout missing")))?;
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, _) = broadcast::channel(128);
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();
        let stderr_tail = Arc::new(std::sync::Mutex::new(
            crate::observability::BoundedTextTail::new(100, 64 * 1024),
        ));
        tokio::spawn(writer_loop(stdin, writer_rx));
        tokio::spawn(reader_loop(stdout, pending.clone(), events_tx.clone()));
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(stderr_loop(stderr, stderr_tail.clone()));
        }
        let host = Self {
            next_req_id: AtomicU64::new(0),
            pending,
            writer_tx,
            events_tx,
            stderr_tail,
            _child: child,
        };
        let init = host
            .request("init", serde_json::json!({ "dbPath": db_path }))
            .await?;
        if !init.ok {
            return Err(ArkHostError::RpcError(init.error.unwrap_or_default()));
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
                m.insert("params".into(), other);
                m
            }
        };
        envelope.insert("operation".into(), operation.into());
        envelope.insert("_req_id".into(), serde_json::Value::String(req_id.clone()));
        let mut line = serde_json::to_vec(&serde_json::Value::Object(envelope))?;
        line.push(b'\n');
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(req_id, tx);
        self.writer_tx
            .send(OutgoingRequest { payload: line })
            .map_err(|_| ArkHostError::ChannelClosed)?;
        rx.await.map_err(|_| ArkHostError::ChildDead)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, serde_json::Value)> {
        self.events_tx.subscribe()
    }
    pub fn stderr_tail_snapshot(&self) -> Vec<String> {
        self.stderr_tail
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot()
    }
}

fn stable_device_id() -> String {
    std::env::var("KOSMOS_DEVICE_ID")
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

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
async fn writer_loop(
    mut stdin: tokio::process::ChildStdin,
    mut rx: mpsc::UnboundedReceiver<OutgoingRequest>,
) {
    while let Some(req) = rx.recv().await {
        if stdin.write_all(&req.payload).await.is_err() || stdin.flush().await.is_err() {
            break;
        }
    }
}
async fn reader_loop(
    stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>>,
    events_tx: broadcast::Sender<(String, serde_json::Value)>,
) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            dispatch_frame(value, &pending, &events_tx).await;
        }
    }
}
async fn stderr_loop(
    stderr: tokio::process::ChildStderr,
    tail: Arc<std::sync::Mutex<crate::observability::BoundedTextTail>>,
) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = crate::observability::redact_log_line(&line);
        tail.lock().unwrap_or_else(|e| e.into_inner()).push(&line);
        eprintln!("[ark-core-rpc] {line}");
    }
}
async fn dispatch_frame(
    value: serde_json::Value,
    pending: &Arc<Mutex<HashMap<String, oneshot::Sender<ArkResponse>>>>,
    events_tx: &broadcast::Sender<(String, serde_json::Value)>,
) {
    if let Some(event) = value.get("event").and_then(|v| v.as_str()) {
        let _ = events_tx.send((event.to_owned(), value));
        return;
    }
    let Some(req_id) = value.get("_req_id").and_then(|v| v.as_str()) else {
        return;
    };
    let response = ArkResponse {
        ok: value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        data: value.get("data").cloned().unwrap_or_default(),
        error: value
            .get("error")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
    };
    if let Some(sender) = pending.lock().await.remove(req_id) {
        let _ = sender.send(response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn dispatch_response_to_pending() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel(8);
        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert("req".into(), tx);
        dispatch_frame(
            serde_json::json!({"_req_id":"req","ok":true,"data":{"result":"foo"}}),
            &pending,
            &events,
        )
        .await;
        assert_eq!(rx.await.unwrap().data["result"], "foo");
    }
}
