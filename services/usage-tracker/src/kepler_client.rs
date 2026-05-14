// Phase 4 (scaffold): WS-клиент к Kepler host из usage-tracker.
//
// Сейчас usage-tracker пишет в ARK DB напрямую через `ark_core::db::upsert_*`.
// Phase 4 — миграция на WS RPC к Kepler host (single sidecar на машине).
//
// Этот модуль — готовая infrastructure. Wiring в `main.rs` — opt-in через env
// `USAGE_TRACKER_USE_KEPLER=1`. Когда AC5 (Phase 4 spec) выполнится — default
// поведение перейдёт на kepler, fallback flag станет deprecation marker.
//
// Lock-file resolution — копирует логику из `@kosmos/ark` `ensureKeplerRunning`,
// но проще (нам не нужен auto-launch — Kepler должен уже быть запущен через
// HKCU autostart / installer).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

const PROTOCOL_VERSION: &str = "1.0.0";
const LOCK_FILE_NAME: &str = "kepler.lock.json";

#[derive(Debug, Clone, Deserialize)]
pub struct KeplerProtocolVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeplerLockInfo {
    pub format_version: u32,
    pub protocol_version: KeplerProtocolVersion,
    pub pid: u32,
    pub ws_port: u16,
    pub auth_token: String,
    pub started_at: String,
    pub db_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum KeplerError {
    #[error("Kepler lock-file not found: {0:?}")]
    LockFileNotFound(PathBuf),
    #[error("Kepler lock-file malformed: {0}")]
    LockFileMalformed(String),
    #[error("Kepler protocol mismatch: server major {server}, client expects {client}")]
    IncompatibleVersion { server: u32, client: u32 },
    #[error("Kepler handshake rejected: code={code} message={message:?}")]
    HandshakeRejected { code: String, message: String },
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("connection closed")]
    Closed,
}

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct KeplerClient {
    runtime: tokio::runtime::Runtime,
    ws: Arc<Mutex<Option<WsStream>>>,
    lock: KeplerLockInfo,
    device_id: String,
    next_req_id: std::sync::atomic::AtomicU64,
}

impl KeplerClient {
    /// Создать client. Соединение lazy — open происходит при первом `invoke`.
    pub fn new(lock: KeplerLockInfo, device_id: String) -> Result<Self, KeplerError> {
        let client_major = 1u32;
        if lock.protocol_version.major != client_major {
            return Err(KeplerError::IncompatibleVersion {
                server: lock.protocol_version.major,
                client: client_major,
            });
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        Ok(Self {
            runtime,
            ws: Arc::new(Mutex::new(None)),
            lock,
            device_id,
            next_req_id: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Sync wrapper: блокирующий вызов в текущем потоке через tokio runtime.
    pub fn invoke(&self, op: &str, params: Value) -> Result<Value, KeplerError> {
        self.runtime.block_on(self.invoke_async(op, params))
    }

    async fn invoke_async(&self, op: &str, params: Value) -> Result<Value, KeplerError> {
        // Один retry с пересозданием соединения, если первый attempt fail'нул на
        // transport-level (закрытое WS, broken pipe и т.п.).
        match self.invoke_once(op, &params).await {
            Ok(v) => Ok(v),
            Err(KeplerError::WebSocket(_)) | Err(KeplerError::Closed) => {
                // Сбрасываем соединение — следующий вызов ensure_connected пересоздаст.
                let mut ws_guard = self.ws.lock().await;
                *ws_guard = None;
                drop(ws_guard);
                self.invoke_once(op, &params).await
            }
            Err(other) => Err(other),
        }
    }

    async fn invoke_once(&self, op: &str, params: &Value) -> Result<Value, KeplerError> {
        self.ensure_connected().await?;

        let req_id = format!(
            "usage-tracker-{}",
            self.next_req_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );

        let mut envelope = match params.clone() {
            Value::Object(map) => map,
            Value::Null => serde_json::Map::new(),
            other => {
                let mut m = serde_json::Map::new();
                m.insert("params".to_string(), other);
                m
            }
        };
        envelope.insert("operation".to_string(), Value::String(op.to_string()));
        envelope.insert("_req_id".to_string(), Value::String(req_id.clone()));

        let payload = serde_json::to_string(&Value::Object(envelope))?;

        let mut ws_guard = self.ws.lock().await;
        let ws = ws_guard.as_mut().ok_or(KeplerError::Closed)?;
        if let Err(e) = ws.send(Message::Text(payload)).await {
            // Соединение могло умереть — reset для retry.
            *ws_guard = None;
            return Err(KeplerError::WebSocket(e));
        }

        // Принимаем frames до response с нашим req_id (events игнорируем).
        loop {
            let frame = match ws.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => {
                    *ws_guard = None;
                    return Err(KeplerError::WebSocket(e));
                }
                None => {
                    *ws_guard = None;
                    return Err(KeplerError::Closed);
                }
            };

            let text = match frame {
                Message::Text(t) => t,
                Message::Close(_) => {
                    *ws_guard = None;
                    return Err(KeplerError::Closed);
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) | Message::Binary(_) => {
                    continue
                }
            };

            let value: Value = serde_json::from_str(&text)?;
            if value.get("event").is_some() && value.get("ok").is_none() {
                // event — игнорим (events на этом этапе не используем).
                continue;
            }

            let resp_id = value.get("_req_id").and_then(|v| v.as_str());
            if resp_id != Some(req_id.as_str()) {
                // Чужой response — игнор, ждём свой.
                continue;
            }

            let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            if !ok {
                let err = value
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no error message)")
                    .to_string();
                return Err(KeplerError::Rpc(err));
            }
            return Ok(value
                .get("data")
                .cloned()
                .unwrap_or(Value::Null));
        }
    }

    async fn ensure_connected(&self) -> Result<(), KeplerError> {
        let mut ws_guard = self.ws.lock().await;
        if ws_guard.is_some() {
            return Ok(());
        }

        let url = format!("ws://127.0.0.1:{}", self.lock.ws_port);
        let (mut ws, _) = connect_async(&url).await?;

        // Hello-handshake.
        let pid = std::process::id();
        let hello = json!({
            "kind": "hello",
            "protocolVersion": PROTOCOL_VERSION,
            "token": self.lock.auth_token,
            "pid": pid,
            "clientId": self.device_id,
        });
        ws.send(Message::Text(hello.to_string())).await?;

        let ack_frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .map_err(|_| KeplerError::Rpc("hello timeout".to_string()))?
            .ok_or(KeplerError::Closed)??;
        let ack_text = match ack_frame {
            Message::Text(t) => t,
            _ => return Err(KeplerError::Rpc("hello expected text frame".to_string())),
        };
        let ack: Value = serde_json::from_str(&ack_text)?;
        if ack.get("kind").and_then(|v| v.as_str()) != Some("hello_ok") {
            let code = ack
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let message = ack
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            return Err(KeplerError::HandshakeRejected { code, message });
        }

        *ws_guard = Some(ws);
        Ok(())
    }

    /// Specialized helpers (mirror direct write functions в main.rs).
    pub fn upsert_tracked_app(&self, tracked_app: &Value, device_id: &str) -> Result<(), KeplerError> {
        let _ = self.invoke(
            "upsert_tracked_app",
            json!({
                "tracked_app": tracked_app,
                "device_id": device_id,
            }),
        )?;
        Ok(())
    }

    pub fn upsert_usage_session(&self, session: &Value, device_id: &str) -> Result<(), KeplerError> {
        let _ = self.invoke(
            "upsert_usage_session",
            json!({
                "usage_session": session,
                "device_id": device_id,
            }),
        )?;
        Ok(())
    }

    pub fn upsert_usage_event(&self, event: &Value, device_id: &str) -> Result<(), KeplerError> {
        let _ = self.invoke(
            "upsert_usage_event",
            json!({
                "usage_event": event,
                "device_id": device_id,
            }),
        )?;
        Ok(())
    }
}

/// Поиск Kepler lock-файла. Возвращает прочитанный KeplerLockInfo если файл есть.
pub fn try_read_kepler_lock() -> Result<KeplerLockInfo, KeplerError> {
    let path = resolve_lock_path()?;
    if !path.exists() {
        return Err(KeplerError::LockFileNotFound(path));
    }
    let bytes = std::fs::read(&path)?;
    let parsed: serde_json::Value = serde_json::from_slice(&bytes)?;
    let lock: KeplerLockInfo =
        serde_json::from_value(parsed).map_err(|e| KeplerError::LockFileMalformed(e.to_string()))?;
    Ok(lock)
}

#[cfg(windows)]
fn resolve_lock_path() -> Result<PathBuf, KeplerError> {
    let appdata = std::env::var("APPDATA").map_err(|_| {
        KeplerError::LockFileMalformed("%APPDATA% env not set".to_string())
    })?;
    Ok(PathBuf::from(appdata).join("Kosmos").join(LOCK_FILE_NAME))
}

#[cfg(not(windows))]
fn resolve_lock_path() -> Result<PathBuf, KeplerError> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("Kosmos").join(LOCK_FILE_NAME));
    }
    let home = std::env::var("HOME").map_err(|_| {
        KeplerError::LockFileMalformed("$HOME not set".to_string())
    })?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("Kosmos")
        .join(LOCK_FILE_NAME))
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct HelloPayload<'a> {
    kind: &'a str,
    #[serde(rename = "protocolVersion")]
    protocol_version: &'a str,
    token: &'a str,
    pid: u32,
    #[serde(rename = "clientId")]
    client_id: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_lock_info() {
        let json = r#"{
            "format_version": 1,
            "protocol_version": { "major": 1, "minor": 0, "patch": 0 },
            "pid": 12345,
            "ws_port": 52436,
            "auth_token": "deadbeef",
            "started_at": "2026-05-13T15:00:00Z",
            "db_path": "C:/path/to/ark.db"
        }"#;
        let lock: KeplerLockInfo = serde_json::from_str(json).unwrap();
        assert_eq!(lock.pid, 12345);
        assert_eq!(lock.ws_port, 52436);
        assert_eq!(lock.protocol_version.major, 1);
    }

    #[test]
    fn try_read_kepler_lock_returns_not_found_for_missing_appdata() {
        // Сохраняем оригинальное значение, ставим заведомо пустое.
        let original = std::env::var("APPDATA").ok();
        std::env::set_var("APPDATA", std::env::temp_dir().join("nonexistent-kosmos-test"));
        let result = try_read_kepler_lock();
        // Восстанавливаем env.
        match original {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }
        assert!(matches!(result, Err(KeplerError::LockFileNotFound(_))));
    }

    #[test]
    fn kepler_client_rejects_incompatible_major() {
        let lock = KeplerLockInfo {
            format_version: 1,
            protocol_version: KeplerProtocolVersion {
                major: 2,
                minor: 0,
                patch: 0,
            },
            pid: 12345,
            ws_port: 52436,
            auth_token: "deadbeef".repeat(8),
            started_at: "2026-05-13T15:00:00Z".to_string(),
            db_path: "C:/path".to_string(),
        };
        let result = KeplerClient::new(lock, "test-device".to_string());
        assert!(matches!(result, Err(KeplerError::IncompatibleVersion { server: 2, client: 1 })));
    }
}
