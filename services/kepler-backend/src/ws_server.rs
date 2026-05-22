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

use crate::app_index::AppIndex;
use crate::ark_host::ArkHost;
use crate::arrancador;
use crate::auth;
use crate::command_bus::{ClientId, CommandBus, CommandBusEvent, CommandManifest};
use crate::export;
use crate::focus::handle_focus_op;
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
    app_index: Arc<AppIndex>,
    next_client_id: Arc<AtomicU64>,
}

impl WsServer {
    /// Биндит TcpListener на 127.0.0.1 + случайный свободный порт.
    /// `data_dir` — куда писать persisted pomodoro state.
    pub async fn bind(
        ark_host: Arc<ArkHost>,
        auth_token: String,
        data_dir: std::path::PathBuf,
        app_index: Arc<AppIndex>,
    ) -> Result<Self, WsServerError> {
        let addr: SocketAddr = "127.0.0.1:0"
            .parse()
            .expect("hardcoded socket literal is always valid");
        let listener = TcpListener::bind(addr).await?;
        Ok(WsServer {
            listener,
            ark_host,
            auth_token: Arc::new(auth_token),
            command_bus: Arc::new(CommandBus::new()),
            pomodoro_host: PomodoroHost::new(data_dir),
            app_index,
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
            let app_idx = self.app_index.clone();
            let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, ark_host, token, bus, pomo, app_idx, client_id).await
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
    app_index: Arc<AppIndex>,
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
    // SDK (@kosmos/ark dispatchSidecarEvent) переключается по полю `event`.
    //
    // Operations с префиксом `commands.` обрабатываются локально через
    // CommandBus, в ark_host не уходят.

    let mut bus_rx = command_bus.subscribe();
    let mut pomo_rx = pomodoro_host.subscribe();
    // Forward ark-core events (object_upserted/object_deleted/entity_changed/peer_*
    // и т.п.) — до 2026-05-20 это broadcast channel был не подключён к WS,
    // events не доходили до клиентов. Cross-app live updates (Eden subscribed
    // на object_upserted для taskRef) полагаются на этот forward.
    let mut ark_evt_rx = ark_host.subscribe_events();

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

            // 2a''. ARK events (object_upserted / object_deleted / entity_changed /
            // peer_* и т.п.) → forward напрямую как wire JSON. Payload уже содержит
            // поле "event" — sink его так и шлёт.
            aevt = ark_evt_rx.recv() => {
                match aevt {
                    Ok((_name, payload)) => {
                        if sink.send(Message::Text(payload.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
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

                // Intercept export.* — Phase 7 universal export dispatch.
                if let Some(rest) = operation.strip_prefix("export.") {
                    let resp = handle_export_op(rest, params, &ark_host).await;
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

                // Intercept arrancador.* — scan + launch (+ rawg.* / sqoba.* через
                // соседние subagent'ы B/C). Read-only части (config get) и write
                // паттерны идут через ark_host где нужно.
                if let Some(rest) = operation.strip_prefix("arrancador.") {
                    let resp = handle_arrancador_op(rest, params, &ark_host).await;
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

                // Intercept focus.* — focus-mode блоклисты (ARK-backed) + active state в sync_kv.
                if let Some(rest) = operation.strip_prefix("focus.") {
                    let resp = handle_focus_op(rest, params, &ark_host).await;
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

                // Intercept app_index.* — app launcher search / launch / rescan.
                if let Some(rest) = operation.strip_prefix("app_index.") {
                    let resp = handle_app_index_op(rest, params, &app_index).await;
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

/// Dispatch `export.<subop>` (Phase 7).
///
/// Sub-operations:
///   - `export.list` → `{ converters: [...] }`
///   - `export.run { converter_id, format?, dest_dir }` → `{ files_written, bytes, errors }`
///
/// Read-only от ARK: fetch objects через `list_objects_by_type`, передаём в
/// converter, который пишет в dest_dir. Никаких writes в ARK.
async fn handle_export_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    match subop {
        "list" => {
            let converters = export::list_converters();
            LocalResponse::ok(serde_json::json!({ "converters": converters }))
        }
        "run" => {
            let converter_id = match params.get("converter_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("export.run: missing 'converter_id'"),
            };
            let dest_dir = match params.get("dest_dir").and_then(|v| v.as_str()) {
                Some(s) => std::path::PathBuf::from(s),
                None => return LocalResponse::err("export.run: missing 'dest_dir'"),
            };
            let converter = match export::find_converter(&converter_id) {
                Some(c) => c,
                None => {
                    return LocalResponse::err(format!(
                        "export.run: unknown converter '{converter_id}'"
                    ))
                }
            };
            let format = params
                .get("format")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| converter.default_format().to_string());
            if !converter
                .supported_formats()
                .iter()
                .any(|f| *f == format.as_str())
            {
                return LocalResponse::err(format!(
                    "export.run: format '{format}' not supported by '{converter_id}'"
                ));
            }

            // Ensure dest_dir exists.
            if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                return LocalResponse::err(format!("export.run: create dest_dir: {e}"));
            }

            // Fetch objects of converter's object_type через ark_host.
            let ark_resp = match ark_host
                .request(
                    "list_objects_by_type",
                    serde_json::json!({ "type_id": converter.object_type() }),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("export.run: ark_host: {e}")),
            };
            if !ark_resp.ok {
                return LocalResponse::err(format!(
                    "export.run: ark list_objects_by_type failed: {}",
                    ark_resp.error.unwrap_or_default()
                ));
            }
            let objects: Vec<ark_core::types::ArkObject> = match serde_json::from_value(ark_resp.data)
            {
                Ok(v) => v,
                Err(e) => {
                    return LocalResponse::err(format!(
                        "export.run: parse ArkObject array: {e}"
                    ))
                }
            };

            let result = converter.convert(&objects, &format, &dest_dir);
            match serde_json::to_value(&result) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("export.run: serialize result: {e}")),
            }
        }
        other => LocalResponse::err(format!("export.{other}: unknown sub-operation")),
    }
}

/// Dispatch `arrancador.<subop>`.
///
/// Sub-operations (subagent A scope):
///   - `arrancador.scan` → сканирует Steam/Epic, upsert'ит game_obj в ARK.
///   - `arrancador.launch { game_id }` → fetch game_obj через ark_host, spawn
///     процесс через `launcher::launch`.
///
/// `arrancador.rawg.*` и `arrancador.sqoba.*` будут добавлены subagent'ами B/C
/// в этот же match (один namespace, один диспатчер).
async fn handle_arrancador_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    match subop {
        "scan" => {
            // Override path — для тестов / non-standard Steam install.
            let override_path: Option<std::path::PathBuf> = params
                .get("steam_library_override")
                .and_then(|v| v.as_str())
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    let cfg = arrancador::config::load();
                    cfg.steam_library_override
                });
            let discovered = arrancador::scanner::scan_all(override_path.as_deref());

            // Fetch existing game_obj для matching по (source, source_app_id).
            let ark_resp = match ark_host
                .request(
                    "list_objects_by_type",
                    serde_json::json!({ "type_id": "game_obj" }),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("arrancador.scan: ark_host: {e}")),
            };
            let existing: Vec<ark_core::types::ArkObject> = if ark_resp.ok {
                match serde_json::from_value(ark_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[arrancador.scan] failed to parse existing games from ARK: {e}");
                        return LocalResponse::err(format!(
                            "arrancador.scan: failed to parse existing games: {e}"
                        ));
                    }
                }
            } else {
                Vec::new()
            };

            let mut added = 0u32;
            let mut updated = 0u32;
            let mut skipped = 0u32;
            let mut errors: Vec<String> = Vec::new();

            for game in &discovered {
                let existing_match = existing.iter().find(|obj| {
                    let src = obj
                        .props_json
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let app_id = obj
                        .props_json
                        .get("source_app_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    src == game.source && app_id == game.source_app_id
                });

                let props = serde_json::json!({
                    "source": game.source,
                    "source_app_id": game.source_app_id,
                    "install_dir": game.install_dir.to_string_lossy(),
                    "exe_path": game.exe_candidate.as_ref().map(|p| p.to_string_lossy().to_string()),
                    "install_size_bytes": game.install_size_bytes,
                    "name": game.name,
                });

                let upsert_obj = if let Some(existing) = existing_match {
                    // Merge: сохраняем content_json + RAWG-метадату которая уже есть.
                    let mut merged_props = existing.props_json.clone();
                    if let Some(map) = merged_props.as_object_mut() {
                        if let Some(new_map) = props.as_object() {
                            for (k, v) in new_map {
                                map.insert(k.clone(), v.clone());
                            }
                        }
                    } else {
                        merged_props = props.clone();
                    }
                    serde_json::json!({
                        "id": existing.id,
                        "type_id": "game_obj",
                        "title": game.name,
                        "content_json": existing.content_json,
                        "props_json": merged_props,
                    })
                } else {
                    serde_json::json!({
                        "type_id": "game_obj",
                        "title": game.name,
                        "content_json": {},
                        "props_json": props,
                    })
                };

                let is_new = existing_match.is_none();
                match ark_host
                    .request("upsert_object", serde_json::json!({ "object": upsert_obj }))
                    .await
                {
                    Ok(r) if r.ok => {
                        if is_new {
                            added += 1;
                        } else {
                            updated += 1;
                        }
                    }
                    Ok(r) => {
                        skipped += 1;
                        errors.push(format!(
                            "{}: upsert failed: {}",
                            game.name,
                            r.error.unwrap_or_default()
                        ));
                    }
                    Err(e) => {
                        skipped += 1;
                        errors.push(format!("{}: ark_host: {e}", game.name));
                    }
                }
            }

            LocalResponse::ok(serde_json::json!({
                "added": added,
                "updated": updated,
                "skipped": skipped,
                "discovered": discovered.len(),
                "errors": errors,
            }))
        }
        "launch" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.launch: missing 'game_id'"),
            };
            let ark_resp = match ark_host
                .request("get_object", serde_json::json!({ "id": game_id }))
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("arrancador.launch: ark_host: {e}")),
            };
            if !ark_resp.ok {
                return LocalResponse::err(format!(
                    "arrancador.launch: get_object failed: {}",
                    ark_resp.error.unwrap_or_default()
                ));
            }
            let game: ark_core::types::ArkObject = match serde_json::from_value(ark_resp.data) {
                Ok(g) => g,
                Err(e) => {
                    return LocalResponse::err(format!(
                        "arrancador.launch: parse ArkObject: {e}"
                    ))
                }
            };
            match arrancador::launcher::launch(&game) {
                Ok(result) => match serde_json::to_value(&result) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => LocalResponse::err(format!("arrancador.launch: serialize: {e}")),
                },
                Err(e) => LocalResponse::err(format!("arrancador.launch: {e}")),
            }
        }
        "config.get" => {
            let cfg = arrancador::config::load();
            // Не возвращаем raw rawg_api_key — только статус.
            let payload = serde_json::json!({
                "rawg_api_key_set": cfg.rawg_api_key.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
                "custom_scan_paths": cfg.custom_scan_paths,
                "sqoba_dest_dir": cfg.sqoba_dest_dir,
                "keep_backups": cfg.keep_backups,
            });
            LocalResponse::ok(payload)
        }
        "config.set_rawg_key" => {
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut cfg = arrancador::config::load();
            cfg.rawg_api_key = key.filter(|s| !s.is_empty());
            match arrancador::config::save(&cfg) {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("arrancador.config.set_rawg_key: {e}")),
            }
        }
        "rawg.search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.rawg.search: missing 'query'"),
            };
            let cfg = arrancador::config::load();
            let api_key = match cfg.rawg_api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return LocalResponse::err("RAWG API key not configured"),
            };
            match arrancador::rawg::search(&query, &api_key).await {
                Ok(results) => match serde_json::to_value(&results) {
                    Ok(v) => LocalResponse::ok(serde_json::json!({ "results": v })),
                    Err(e) => {
                        LocalResponse::err(format!("arrancador.rawg.search: serialize: {e}"))
                    }
                },
                Err(e) => LocalResponse::err(format!("arrancador.rawg.search: {e}")),
            }
        }
        "rawg.apply" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.rawg.apply: missing 'game_id'"),
            };
            let rawg_id = match params.get("rawg_id").and_then(|v| v.as_u64()) {
                Some(n) => match u32::try_from(n) {
                    Ok(id) => id,
                    Err(_) => {
                        return LocalResponse::err(format!(
                            "arrancador.rawg.apply: rawg_id {n} exceeds u32 range"
                        ))
                    }
                },
                None => return LocalResponse::err("arrancador.rawg.apply: missing 'rawg_id'"),
            };
            let cfg = arrancador::config::load();
            let api_key = match cfg.rawg_api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return LocalResponse::err("RAWG API key not configured"),
            };
            match arrancador::rawg::apply_to_game_obj(ark_host, &game_id, rawg_id, &api_key).await {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("arrancador.rawg.apply: {e}")),
            }
        }
        "sqoba.backup" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.backup: missing 'game_id'"),
            };
            let ark_resp = match ark_host
                .request("get_object", serde_json::json!({ "id": game_id }))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    return LocalResponse::err(format!("arrancador.sqoba.backup: ark_host: {e}"))
                }
            };
            if !ark_resp.ok {
                return LocalResponse::err(format!(
                    "arrancador.sqoba.backup: get_object failed: {}",
                    ark_resp.error.unwrap_or_default()
                ));
            }
            let game: ark_core::types::ArkObject = match serde_json::from_value(ark_resp.data) {
                Ok(g) => g,
                Err(e) => {
                    return LocalResponse::err(format!(
                        "arrancador.sqoba.backup: parse ArkObject: {e}"
                    ))
                }
            };
            // Имя берём из ArkObject.title (canonical), fallback — props_json.name.
            let game_name = if !game.title.is_empty() {
                game.title.clone()
            } else {
                game.props_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&game.id)
                    .to_string()
            };
            // Manual save paths из propsJson.save_paths (массив строк) если есть.
            let manual_paths: Option<Vec<std::path::PathBuf>> = game
                .props_json
                .get("save_paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(std::path::PathBuf::from))
                        .collect()
                });
            match arrancador::sqoba::backup(
                &game_id,
                &game_name,
                manual_paths.as_deref(),
            ) {
                Ok(b) => match serde_json::to_value(&b) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => {
                        LocalResponse::err(format!("arrancador.sqoba.backup: serialize: {e}"))
                    }
                },
                Err(e) => LocalResponse::err(format!("arrancador.sqoba.backup: {e}")),
            }
        }
        "sqoba.list" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.list: missing 'game_id'"),
            };
            let backups = arrancador::sqoba::list_backups(&game_id);
            match serde_json::to_value(&backups) {
                Ok(v) => LocalResponse::ok(serde_json::json!({ "backups": v })),
                Err(e) => LocalResponse::err(format!("arrancador.sqoba.list: serialize: {e}")),
            }
        }
        "sqoba.restore" => {
            let backup_id = match params.get("backup_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.restore: missing 'backup_id'"),
            };
            // game_id опционален; если задан — резолвим через list_backups,
            // иначе принимаем backup_id как уже полный path.
            let path = if let Some(game_id) = params.get("game_id").and_then(|v| v.as_str()) {
                match arrancador::sqoba::resolve_backup_path(game_id, &backup_id) {
                    Some(p) => p,
                    None => {
                        return LocalResponse::err(format!(
                            "arrancador.sqoba.restore: backup '{}' not found for game '{}'",
                            backup_id, game_id
                        ))
                    }
                }
            } else {
                std::path::PathBuf::from(&backup_id)
            };
            match arrancador::sqoba::restore(&path) {
                Ok(r) => match serde_json::to_value(&r) {
                    Ok(v) => LocalResponse::ok(v),
                    Err(e) => {
                        LocalResponse::err(format!("arrancador.sqoba.restore: serialize: {e}"))
                    }
                },
                Err(e) => LocalResponse::err(format!("arrancador.sqoba.restore: {e}")),
            }
        }
        other => LocalResponse::err(format!("arrancador.{other}: unknown sub-operation")),
    }
}

/// Dispatch `app_index.<subop>` — App Launcher: search / launch / rescan.
///
/// Sub-operations:
///   - `app_index.search { query, limit? }` → `{ results: [{id, name, icon_path, kind, score}] }`
///   - `app_index.launch { id }` → `{ ok: true }`. Frecency tracking — TODO через ARK usage_event_obj.
///   - `app_index.rescan` → `{ added, updated, removed, total }`
async fn handle_app_index_op(
    subop: &str,
    params: serde_json::Value,
    app_index: &Arc<AppIndex>,
) -> LocalResponse {
    use crate::app_index::ranking::UsageStats;

    match subop {
        "list_all" => {
            // Все приложения с inline base64 иконками. Используется когда
            // launcher хочет показать apps как часть общего списка команд
            // (без отдельной поисковой подсекции).
            let limit = params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(500) as usize;
            let mut out: Vec<crate::app_index::App> = app_index.all(limit).await;
            for app in &mut out {
                if let Some(path) = app.icon_path.clone() {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let b64 = base64_encode(&bytes);
                        app.icon_path = Some(format!("data:image/png;base64,{b64}"));
                    } else {
                        app.icon_path = None;
                    }
                }
            }
            match serde_json::to_value(serde_json::json!({ "apps": out })) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("app_index.list_all: serialize: {e}")),
            }
        }
        "search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(q) => q.to_string(),
                None => return LocalResponse::err("app_index.search: missing 'query'"),
            };
            let limit = params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(8) as usize;
            // Frecency: пустой UsageStats в v1. TODO: join из ARK usage_event_obj.
            let usage = UsageStats::empty();
            let mut results = app_index.search(&query, limit, &usage).await;
            // Inline icon как data URL — renderer не имеет file:// доступа.
            // ~50KB per icon, top-8 = ~400KB JSON, приемлемо.
            for scored in &mut results {
                if let Some(path) = scored.app.icon_path.clone() {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let b64 =
                            base64_encode(&bytes);
                        scored.app.icon_path = Some(format!("data:image/png;base64,{b64}"));
                    } else {
                        scored.app.icon_path = None;
                    }
                }
            }
            match serde_json::to_value(serde_json::json!({ "results": results })) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("app_index.search: serialize: {e}")),
            }
        }
        "launch" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("app_index.launch: missing 'id'"),
            };
            let app = match app_index.find(&id).await {
                Some(a) => a,
                None => return LocalResponse::err(format!("app_index.launch: not found: {id}")),
            };
            match app_index.launch(&app) {
                Ok(_) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("app_index.launch: {e}")),
            }
        }
        "rescan" => match app_index.rescan().await {
            Ok(stats) => match serde_json::to_value(&stats) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("app_index.rescan: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("app_index.rescan: {e}")),
        },
        other => LocalResponse::err(format!("app_index.{other}: unknown sub-operation")),
    }
}

/// Minimal base64 encoder (RFC 4648 standard alphabet, no padding-stripping).
/// Используется для inline PNG icons в app_index.search response. Не добавляем
/// dep `base64` ради ~30 строк.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let n = (u32::from(input[i]) << 16) | (u32::from(input[i + 1]) << 8) | u32::from(input[i + 2]);
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        out.push(ALPHABET[(n & 0x3F) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = u32::from(input[i]) << 16;
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = (u32::from(input[i]) << 16) | (u32::from(input[i + 1]) << 8);
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
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
