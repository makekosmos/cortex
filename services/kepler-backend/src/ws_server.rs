// WebSocket server: принимает client-апки (Electron + usage-tracker), валидирует
// hello-handshake (token + version + PID), маршрутизирует JSON-RPC запросы в ark_host.
//
// AC1 (часть): все ARK-операции работают через WS.
// AC4: версионный handshake — отказ без protocolVersion / с MAJOR mismatch.
// AC5: PID-binding — отказ, если PID не существует или принадлежит другому user'у.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use crate::ark_host::ArkHost;
use crate::auth;
use crate::command_bus::{ClientId, CommandBus, CommandBusEvent, CommandManifest};
use crate::pomodoro_host::{handle_pomodoro_op, PomodoroHost};
use crate::protocol_version::{Compatibility, ProtocolVersion, PROTOCOL_VERSION};

/// Закрывающие коды (соответствуют codes в hello-error response).
pub mod handshake_errors {
    pub const MISSING_PROTOCOL_VERSION: &str = "missing_protocol_version";
    pub const MALFORMED_PROTOCOL_VERSION: &str = "malformed_protocol_version";
    pub const INCOMPATIBLE_PROTOCOL_VERSION: &str = "incompatible_protocol_version";
    pub const MISSING_TOKEN: &str = "missing_token";
    pub const INVALID_TOKEN: &str = "invalid_token";
    pub const MISSING_PID: &str = "missing_pid";
    pub const INVALID_PID: &str = "invalid_pid";
    pub const FOREIGN_USER_PID: &str = "foreign_user_pid";
    pub const MALFORMED_HELLO: &str = "malformed_hello";
}

#[derive(Debug, Error)]
pub enum WsServerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    #[serde(default)]
    pub kind: Option<String>, // ожидаем "hello"
    #[serde(rename = "protocolVersion")]
    pub protocol_version: Option<String>,
    pub token: Option<String>,
    pub pid: Option<u32>,
    #[serde(rename = "clientId", default)]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelloOkResponse<'a> {
    pub kind: &'static str, // "hello_ok"
    #[serde(rename = "protocolVersion")]
    pub protocol_version: &'a str,
    pub compatibility: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelloErrorResponse<'a> {
    pub kind: &'static str, // "hello_error"
    pub code: &'a str,
    pub message: String,
}

/// Результат валидации hello — что отправить клиенту перед основным циклом.
#[derive(Debug, Clone)]
pub enum HelloOutcome {
    Accept(Compatibility),
    Reject {
        code: &'static str,
        message: String,
    },
}

/// Чистая функция (детерминированная) — отделена от network IO, тестируется легко.
pub fn validate_hello(hello: &HelloMessage, expected_token: &str) -> HelloOutcome {
    let raw_version = match &hello.protocol_version {
        Some(v) => v,
        None => {
            return HelloOutcome::Reject {
                code: handshake_errors::MISSING_PROTOCOL_VERSION,
                message: "client must send protocolVersion in hello".into(),
            }
        }
    };

    let parsed_version = match ProtocolVersion::parse(raw_version) {
        Ok(v) => v,
        Err(e) => {
            return HelloOutcome::Reject {
                code: handshake_errors::MALFORMED_PROTOCOL_VERSION,
                message: format!("invalid protocolVersion {raw_version:?}: {e}"),
            }
        }
    };

    let compatibility = parsed_version.is_compatible_with_server(&ProtocolVersion::CURRENT);
    if matches!(compatibility, Compatibility::Incompatible) {
        return HelloOutcome::Reject {
            code: handshake_errors::INCOMPATIBLE_PROTOCOL_VERSION,
            message: format!(
                "client protocol MAJOR={} differs from server MAJOR={}",
                parsed_version.major,
                ProtocolVersion::CURRENT.major
            ),
        };
    }

    let token = match &hello.token {
        Some(t) => t,
        None => {
            return HelloOutcome::Reject {
                code: handshake_errors::MISSING_TOKEN,
                message: "client must send auth token in hello".into(),
            }
        }
    };

    if !auth::validate_token(token, expected_token) {
        return HelloOutcome::Reject {
            code: handshake_errors::INVALID_TOKEN,
            message: "auth token does not match server's lock-file token".into(),
        };
    }

    let pid = match hello.pid {
        Some(p) => p,
        None => {
            return HelloOutcome::Reject {
                code: handshake_errors::MISSING_PID,
                message: "client must send its OS PID for PID-binding".into(),
            }
        }
    };

    match auth::validate_pid_belongs_to_current_user(pid) {
        Ok(()) => HelloOutcome::Accept(compatibility),
        Err(auth::AuthError::PidNotFound { .. }) => HelloOutcome::Reject {
            code: handshake_errors::INVALID_PID,
            message: format!("PID {pid} does not exist on this machine"),
        },
        Err(auth::AuthError::ForeignUserPid { .. }) => HelloOutcome::Reject {
            code: handshake_errors::FOREIGN_USER_PID,
            message: format!("PID {pid} belongs to another user account"),
        },
        Err(other) => HelloOutcome::Reject {
            code: handshake_errors::INVALID_PID,
            message: format!("PID-binding check failed: {other}"),
        },
    }
}

pub fn compatibility_label(c: &Compatibility) -> &'static str {
    match c {
        Compatibility::Exact => "exact",
        Compatibility::MinorMismatch => "minor_mismatch",
        Compatibility::Incompatible => "incompatible",
    }
}

pub struct WsServer {
    listener: TcpListener,
    ark_host: Arc<ArkHost>,
    auth_token: Arc<String>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    next_client_id: Arc<AtomicU64>,
}

impl WsServer {
    /// Биндит TcpListener на 127.0.0.1 + случайный свободный порт.
    pub async fn bind(ark_host: Arc<ArkHost>, auth_token: String) -> Result<Self, WsServerError> {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(addr).await?;
        Ok(WsServer {
            listener,
            ark_host,
            auth_token: Arc::new(auth_token),
            command_bus: Arc::new(CommandBus::new()),
            pomodoro_host: PomodoroHost::new(),
            next_client_id: Arc::new(AtomicU64::new(1)),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, WsServerError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or_default()
    }

    /// Главный accept loop. Spawn'ит per-connection task. Завершается, если
    /// listener закрыт (например, через graceful shutdown).
    pub async fn run(self) -> Result<(), WsServerError> {
        loop {
            let (stream, _peer) = self.listener.accept().await?;
            let ark_host = self.ark_host.clone();
            let token = self.auth_token.clone();
            let bus = self.command_bus.clone();
            let pomo = self.pomodoro_host.clone();
            let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, ark_host, token, bus, pomo, client_id).await
                {
                    eprintln!("[kepler.ws] connection error: {e}");
                }
            });
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    ark_host: Arc<ArkHost>,
    expected_token: Arc<String>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    client_id: ClientId,
) -> Result<(), WsServerError> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut stream) = ws.split();

    // 1. Hello.
    let hello_msg = match stream.next().await {
        Some(Ok(Message::Text(text))) => text,
        Some(Ok(Message::Binary(_))) | Some(Ok(_)) => {
            send_hello_error(
                &mut sink,
                handshake_errors::MALFORMED_HELLO,
                "first frame must be text JSON hello",
            )
            .await?;
            return Ok(());
        }
        Some(Err(e)) => return Err(e.into()),
        None => return Ok(()),
    };

    let hello: HelloMessage = match serde_json::from_str(&hello_msg) {
        Ok(h) => h,
        Err(e) => {
            send_hello_error(
                &mut sink,
                handshake_errors::MALFORMED_HELLO,
                &format!("hello JSON parse failed: {e}"),
            )
            .await?;
            return Ok(());
        }
    };

    match validate_hello(&hello, &expected_token) {
        HelloOutcome::Reject { code, message } => {
            send_hello_error(&mut sink, code, &message).await?;
            return Ok(());
        }
        HelloOutcome::Accept(compat) => {
            let response = HelloOkResponse {
                kind: "hello_ok",
                protocol_version: PROTOCOL_VERSION,
                compatibility: compatibility_label(&compat),
            };
            let payload = serde_json::to_string(&response)?;
            sink.send(Message::Text(payload)).await?;
        }
    }

    // 2. Основной цикл. Каждый frame от клиента — JSON-RPC request:
    //    {"operation": "...", "id": "...", ...params}
    // Ответ:
    //    {"id": "...", "ok": ..., "data"?, "error"?}
    //
    // Также слушаем `command_bus` broadcast и форвардим events клиенту в том же
    // wire-формате что и существующие ARK события (peer_connected/entity_changed):
    //   {"event":"commands_changed","commands":[...]}
    //   {"event":"command_invoked","id":...,"params":...}
    // SDK (@kepler/ark dispatchSidecarEvent) переключается по полю `event`.
    //
    // Operations с префиксом `commands.` обрабатываются локально через
    // CommandBus, в ark_host не уходят.

    let mut bus_rx = command_bus.subscribe();
    let mut pomo_rx = pomodoro_host.subscribe();

    loop {
        tokio::select! {
            biased;

            // 2a. Outgoing: events from command_bus → client.
            evt = bus_rx.recv() => {
                match evt {
                    Ok(CommandBusEvent::Changed(list)) => {
                        let payload = serde_json::json!({
                            "event": "commands_changed",
                            "commands": list,
                        });
                        if sink.send(Message::Text(payload.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Ok(CommandBusEvent::Invoked { id, params }) => {
                        let payload = serde_json::json!({
                            "event": "command_invoked",
                            "id": id,
                            "params": params,
                        });
                        if sink.send(Message::Text(payload.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Resubscribe-friendly: drop the lagged event, continue.
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2a'. Pomodoro events → forward as wire-formatted JSON.
            pevt = pomo_rx.recv() => {
                match pevt {
                    Ok(payload) => {
                        if sink.send(Message::Text(payload.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2b. Incoming: WS frame from client → dispatch.
            frame = stream.next() => {
                let frame = match frame {
                    Some(f) => f,
                    None => break,
                };

                let text = match frame {
                    Ok(Message::Text(t)) => t,
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(p)) => {
                        let _ = sink.send(Message::Pong(p)).await;
                        continue;
                    }
                    Ok(_) => continue, // binary/pong — игнор
                    Err(_) => break,
                };

                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = sink
                            .send(Message::Text(format!(
                                r#"{{"ok":false,"error":"malformed JSON: {e}"}}"#
                            )))
                            .await;
                        continue;
                    }
                };

                // Envelope-id: SDK шлёт `_req_id` (новое), legacy clients — `id`.
                // КРИТИЧНО: если есть `_req_id`, payload-поле `id` оставляем как
                // есть — оно принадлежит операции (get_object {id}, delete_object {id}
                // и т.п.). Иначе serde в ark-core-rpc отвалится с "missing field `id`".
                let has_req_id_field = value.get("_req_id").and_then(|v| v.as_str()).is_some();
                let req_id = if has_req_id_field {
                    value
                        .get("_req_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    value
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                };
                let operation = match value.get("operation").and_then(|v| v.as_str()) {
                    Some(op) => op.to_string(),
                    None => {
                        let response = serde_json::json!({
                            "id": req_id,
                            "ok": false,
                            "error": "missing 'operation' field"
                        });
                        let _ = sink.send(Message::Text(response.to_string())).await;
                        continue;
                    }
                };

                // Передаём всё кроме envelope-полей и `operation` в params. Когда
                // envelope-id живёт в `_req_id`, payload `id` НЕ трогаем — это
                // legitimate поле операции.
                let mut params = value.clone();
                if let Some(map) = params.as_object_mut() {
                    map.remove("_req_id");
                    map.remove("operation");
                    if !has_req_id_field {
                        map.remove("id");
                    }
                }

                // Intercept pomodoro.* — обрабатываем локально через PomodoroHost.
                if let Some(rest) = operation.strip_prefix("pomodoro.") {
                    let resp = handle_pomodoro_op(rest, params, &pomodoro_host).await;
                    let mut envelope = serde_json::Map::new();
                    if let Some(id) = req_id {
                        envelope.insert("id".into(), serde_json::Value::String(id));
                    }
                    envelope.insert("ok".into(), serde_json::Value::Bool(resp.ok));
                    envelope.insert("data".into(), resp.data);
                    if let Some(err) = resp.error {
                        envelope.insert("error".into(), serde_json::Value::String(err));
                    }
                    let payload = serde_json::Value::Object(envelope).to_string();
                    if sink.send(Message::Text(payload)).await.is_err() {
                        break;
                    }
                    continue;
                }

                // Intercept commands.* — обрабатываем локально.
                if let Some(rest) = operation.strip_prefix("commands.") {
                    let resp =
                        handle_command_op(rest, params, &command_bus, client_id).await;
                    let mut envelope = serde_json::Map::new();
                    if let Some(id) = req_id {
                        envelope.insert("id".into(), serde_json::Value::String(id));
                    }
                    envelope.insert("ok".into(), serde_json::Value::Bool(resp.ok));
                    envelope.insert("data".into(), resp.data);
                    if let Some(err) = resp.error {
                        envelope.insert("error".into(), serde_json::Value::String(err));
                    }
                    let payload = serde_json::Value::Object(envelope).to_string();
                    if sink.send(Message::Text(payload)).await.is_err() {
                        break;
                    }
                    continue;
                }

                match ark_host.request(&operation, params).await {
                    Ok(ark_response) => {
                        let mut envelope = serde_json::Map::new();
                        if let Some(id) = req_id {
                            envelope.insert("id".into(), serde_json::Value::String(id));
                        }
                        envelope.insert("ok".into(), serde_json::Value::Bool(ark_response.ok));
                        envelope.insert("data".into(), ark_response.data);
                        if let Some(err) = ark_response.error {
                            envelope.insert("error".into(), serde_json::Value::String(err));
                        }
                        let payload = serde_json::Value::Object(envelope).to_string();
                        if sink.send(Message::Text(payload)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let response = serde_json::json!({
                            "id": req_id,
                            "ok": false,
                            "error": format!("ark_host: {e}")
                        });
                        if sink.send(Message::Text(response.to_string())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    // Disconnect: drop client's commands, notify everyone else.
    command_bus.unregister_all(client_id).await;
    command_bus.broadcast_changed().await;

    Ok(())
}

/// Локальный response от `commands.*` обработчика. Та же форма что у
/// `ArkResponse`, но конструируется без обращения к ark-core-rpc.
struct LocalResponse {
    ok: bool,
    data: serde_json::Value,
    error: Option<String>,
}

impl LocalResponse {
    fn ok(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }

    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: serde_json::Value::Null,
            error: Some(msg.into()),
        }
    }
}

/// Диспатч `commands.<subop>` — обрабатывает register / unregister / list /
/// invoke, эмитит broadcast events где нужно.
async fn handle_command_op(
    subop: &str,
    params: serde_json::Value,
    bus: &CommandBus,
    client_id: ClientId,
) -> LocalResponse {
    match subop {
        "register" => {
            let manifests = match params.get("commands") {
                Some(v) => match serde_json::from_value::<Vec<CommandManifest>>(v.clone()) {
                    Ok(m) => m,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "commands.register: invalid 'commands' array: {e}"
                        ))
                    }
                },
                None => return LocalResponse::err("commands.register: missing 'commands' array"),
            };
            bus.register(client_id, manifests).await;
            bus.broadcast_changed().await;
            LocalResponse::ok(serde_json::json!({ "ok": true }))
        }
        "unregister" => {
            let ids = match params.get("ids") {
                Some(v) => match serde_json::from_value::<Vec<String>>(v.clone()) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "commands.unregister: invalid 'ids' array: {e}"
                        ))
                    }
                },
                None => return LocalResponse::err("commands.unregister: missing 'ids' array"),
            };
            bus.unregister(client_id, &ids).await;
            bus.broadcast_changed().await;
            LocalResponse::ok(serde_json::json!({ "ok": true }))
        }
        "list" => {
            let list = bus.list().await;
            LocalResponse::ok(serde_json::json!({ "commands": list }))
        }
        "invoke" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("commands.invoke: missing 'id'"),
            };
            let invoke_params = params
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            bus.broadcast_invoked(id, invoke_params);
            LocalResponse::ok(serde_json::json!({ "ok": true }))
        }
        other => LocalResponse::err(format!("commands.{other}: unknown sub-operation")),
    }
}

async fn send_hello_error<S>(
    sink: &mut S,
    code: &str,
    message: &str,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let payload = serde_json::to_string(&HelloErrorResponse {
        kind: "hello_error",
        code,
        message: message.to_string(),
    })?;
    sink.send(Message::Text(payload)).await?;
    let _ = sink.send(Message::Close(None)).await;
    Ok(())
}

// ----- Unit tests: hello validation без network -----
#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_hello() -> HelloMessage {
        HelloMessage {
            kind: Some("hello".into()),
            protocol_version: Some(PROTOCOL_VERSION.into()),
            token: Some("test-token".into()),
            pid: Some(std::process::id()),
            client_id: Some("eden".into()),
        }
    }

    #[test]
    fn valid_hello_accepted() {
        let hello = baseline_hello();
        let outcome = validate_hello(&hello, "test-token");
        match outcome {
            HelloOutcome::Accept(c) => assert_eq!(c, Compatibility::Exact),
            HelloOutcome::Reject { code, message } => panic!("rejected: {code} {message}"),
        }
    }

    #[test]
    fn missing_protocol_version_rejected() {
        let mut hello = baseline_hello();
        hello.protocol_version = None;
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::MISSING_PROTOCOL_VERSION);
            }
            HelloOutcome::Accept(_) => panic!("should reject"),
        }
    }

    #[test]
    fn malformed_protocol_version_rejected() {
        let mut hello = baseline_hello();
        hello.protocol_version = Some("not-a-version".into());
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::MALFORMED_PROTOCOL_VERSION);
            }
            HelloOutcome::Accept(_) => panic!("should reject"),
        }
    }

    #[test]
    fn major_mismatch_rejected_as_incompatible() {
        let mut hello = baseline_hello();
        hello.protocol_version = Some("2.0.0".into());
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::INCOMPATIBLE_PROTOCOL_VERSION);
            }
            HelloOutcome::Accept(_) => panic!("MAJOR mismatch must be rejected"),
        }
    }

    #[test]
    fn minor_mismatch_accepted() {
        let mut hello = baseline_hello();
        hello.protocol_version = Some("1.99.0".into());
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Accept(c) => assert_eq!(c, Compatibility::MinorMismatch),
            HelloOutcome::Reject { code, message } => panic!("rejected: {code} {message}"),
        }
    }

    #[test]
    fn missing_token_rejected() {
        let mut hello = baseline_hello();
        hello.token = None;
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::MISSING_TOKEN);
            }
            HelloOutcome::Accept(_) => panic!("should reject"),
        }
    }

    #[test]
    fn invalid_token_rejected() {
        let mut hello = baseline_hello();
        hello.token = Some("wrong-token".into());
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::INVALID_TOKEN);
            }
            HelloOutcome::Accept(_) => panic!("should reject"),
        }
    }

    #[test]
    fn missing_pid_rejected() {
        let mut hello = baseline_hello();
        hello.pid = None;
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::MISSING_PID);
            }
            HelloOutcome::Accept(_) => panic!("should reject"),
        }
    }

    #[test]
    fn nonexistent_pid_rejected() {
        let mut hello = baseline_hello();
        hello.pid = Some(0x7FFFFFFF); // impossibly high
        match validate_hello(&hello, "test-token") {
            HelloOutcome::Reject { code, .. } => {
                assert_eq!(code, handshake_errors::INVALID_PID);
            }
            HelloOutcome::Accept(_) => panic!("should reject"),
        }
    }

    #[test]
    fn compatibility_label_strings() {
        assert_eq!(compatibility_label(&Compatibility::Exact), "exact");
        assert_eq!(
            compatibility_label(&Compatibility::MinorMismatch),
            "minor_mismatch"
        );
        assert_eq!(
            compatibility_label(&Compatibility::Incompatible),
            "incompatible"
        );
    }
}
