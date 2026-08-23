// WebSocket server: принимает client-апки (Electron + usage-tracker), валидирует
// hello-handshake (token + version + PID), маршрутизирует JSON-RPC запросы в ark_host.
//
// AC1 (часть): все ARK-операции работают через WS.
// AC4: версионный handshake — отказ без protocolVersion / с MAJOR mismatch.
// AC5: PID-binding — отказ, если PID не существует или принадлежит другому user'у.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use crate::agents::AgentsService;
use crate::app_index::AppIndex;
use crate::ark_host::ArkHost;
use crate::arrancador;
use crate::auth;
use crate::calculator;
use crate::command_bus::{ClientId, CommandBus, CommandBusEvent, CommandManifest};
use crate::diagnostics::{RpcDiagnostics, SharedRpcDiagnostics};
use crate::dictation::{handle_dictation_op, DictationHost};
use crate::export;
use crate::file_index::{FileIndex, FileIndexSettingsPatch};
use crate::focus::handle_focus_op;
use crate::grant_authority::{GrantAuthorityRegistry, GrantOwner, GrantProvenance};
use crate::integrations;
use crate::manager_api::ManagerState;
use crate::package_service::{PackageError, PackageService};
use crate::package_trust::{SignatureSet, TrustError};
use crate::pomodoro_host::{handle_pomodoro_op, PomodoroHost};
use crate::protocol_usage::{ProtocolUsageStore, TransportKind};
use crate::protocol_version::{Compatibility, ProtocolVersion, API_VERSION, API_VERSION_CURRENT};
use crate::store_catalog::{CatalogDto, PackageIndexLookup, StoreCatalogService};
use crate::usage_tracker::UsageTrackerDiagnosticsState;
use base64::Engine as _;

#[path = "ws_server_ops.rs"]
mod ws_server_ops;
use ws_server_ops::*;

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
    pub const AMBIGUOUS_VERSION: &str = "ambiguous_version";
    pub const UPGRADE_REQUIRED: &str = "upgrade_required";
}

// WAV is sent as base64 JSON today. 16 MiB covers a five-minute 16 kHz mono
// recording while keeping a bounded authenticated-local transport limit.
// См. postmortems.md § 2026-08-19.
const MAX_WS_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ACTIVE_WS_CONNECTIONS: usize = 128;
const MAX_ACTIVE_WS_REQUESTS: usize = 128;
const WS_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);
const WS_SEND_DEADLINE: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum WsServerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

struct WsLifecycle {
    admission: Mutex<()>,
    closed: AtomicBool,
    shutdown: tokio::sync::Notify,
    capacity: Arc<tokio::sync::Semaphore>,
    tasks: Mutex<HashMap<u64, WsConnectionSlot>>,
    next_task: AtomicU64,
    request_closed: AtomicBool,
    request_capacity: Arc<tokio::sync::Semaphore>,
    request_tasks: Mutex<HashMap<u64, WsRequestSlot>>,
    next_request: AtomicU64,
    response_deadline: Mutex<Duration>,
}

struct WsConnectionResources {
    stream: Option<tokio::net::TcpStream>,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    owner_lease: Option<crate::engine_dispatch::OwnerLease>,
}

enum WsConnectionSlot {
    Reserved {
        resources: Arc<Mutex<Option<WsConnectionResources>>>,
        start: tokio::sync::oneshot::Sender<()>,
    },
    Installed(tokio::task::JoinHandle<()>),
}

enum WsRequestSlot {
    Reserved {
        permit: Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>,
        cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    },
    Installed {
        task: tokio::task::JoinHandle<()>,
        cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    },
}

impl Default for WsLifecycle {
    fn default() -> Self {
        Self {
            admission: Mutex::new(()),
            closed: AtomicBool::new(false),
            shutdown: tokio::sync::Notify::new(),
            capacity: Arc::new(tokio::sync::Semaphore::new(MAX_ACTIVE_WS_CONNECTIONS)),
            tasks: Mutex::new(HashMap::new()),
            next_task: AtomicU64::new(1),
            request_closed: AtomicBool::new(false),
            request_capacity: Arc::new(tokio::sync::Semaphore::new(MAX_ACTIVE_WS_REQUESTS)),
            request_tasks: Mutex::new(HashMap::new()),
            next_request: AtomicU64::new(1),
            response_deadline: Mutex::new(Duration::from_secs(30)),
        }
    }
}

#[derive(Clone)]
pub struct WsShutdownHandle {
    lifecycle: Arc<WsLifecycle>,
}

impl WsShutdownHandle {
    pub async fn begin_shutdown(&self) {
        let _admission = self
            .lifecycle
            .admission
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if !self.lifecycle.closed.swap(true, Ordering::AcqRel) {
            self.lifecycle.request_closed.store(true, Ordering::Release);
            self.lifecycle.shutdown.notify_waiters();
        }
    }

    async fn cancelled(&self) {
        let notified = self.lifecycle.shutdown.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.lifecycle.closed.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }

    async fn reap(&self) {
        let finished = {
            let mut tasks = self
                .lifecycle
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let ids = tasks
                .iter()
                .filter_map(|(id, slot)| match slot {
                    WsConnectionSlot::Installed(task) if task.is_finished() => Some(*id),
                    WsConnectionSlot::Reserved { .. } | WsConnectionSlot::Installed(_) => None,
                })
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| tasks.remove(&id))
                .collect::<Vec<_>>()
        };
        for slot in finished {
            if let WsConnectionSlot::Installed(task) = slot {
                let _ = task.await;
            }
        }
        let finished_requests = {
            let mut tasks = self
                .lifecycle
                .request_tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let ids = tasks
                .iter()
                .filter_map(|(id, slot)| match slot {
                    WsRequestSlot::Installed { task, .. } if task.is_finished() => Some(*id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            ids.into_iter()
                .filter_map(|id| tasks.remove(&id))
                .collect::<Vec<_>>()
        };
        for slot in finished_requests {
            if let WsRequestSlot::Installed { task, .. } = slot {
                let _ = task.await;
            }
        }
    }

    async fn finish_request(&self, request_id: u64, cancel: bool) {
        let slot = self
            .lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&request_id);
        let Some(slot) = slot else {
            return;
        };
        match slot {
            WsRequestSlot::Reserved {
                permit,
                cancel: sender,
            } => {
                let _ = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                if let Some(sender) = sender.lock().unwrap_or_else(|p| p.into_inner()).take() {
                    let _ = sender.send(());
                }
            }
            WsRequestSlot::Installed {
                task,
                cancel: sender,
            } => {
                if cancel {
                    if let Some(sender) = sender.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = sender.send(());
                    }
                    let mut task = task;
                    if tokio::time::timeout(WS_SEND_DEADLINE, &mut task)
                        .await
                        .is_err()
                    {
                        task.abort();
                        let _ = task.await;
                    }
                } else {
                    let _ = task.await;
                }
            }
        }
    }

    fn install_request<F>(
        &self,
        request_id: u64,
        permit: Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>,
        cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
        spawn: F,
    ) -> bool
    where
        F: FnOnce(tokio::sync::oneshot::Receiver<()>) -> tokio::task::JoinHandle<()>,
    {
        let _admission = self
            .lifecycle
            .admission
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if self.lifecycle.request_closed.load(Ordering::Acquire) {
            let _ = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
            return false;
        }
        self.lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                request_id,
                WsRequestSlot::Reserved {
                    permit: permit.clone(),
                    cancel: cancel.clone(),
                },
            );
        let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
        let task = spawn(start_receiver);
        let old = self
            .lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(request_id, WsRequestSlot::Installed { task, cancel });
        debug_assert!(matches!(old, Some(WsRequestSlot::Reserved { .. })));
        let _ = start_sender.send(());
        true
    }

    pub fn task_count(&self) -> usize {
        self.lifecycle
            .tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    pub fn request_task_count(&self) -> usize {
        self.lifecycle
            .request_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    pub fn available_capacity(&self) -> usize {
        self.lifecycle.capacity.available_permits()
    }

    fn response_deadline(&self) -> Duration {
        *self
            .lifecycle
            .response_deadline
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    #[cfg(test)]
    fn set_response_deadline(&self, deadline: Duration) {
        *self
            .lifecycle
            .response_deadline
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = deadline;
    }

    async fn drain(&self, deadline: Duration) -> Result<(), &'static str> {
        self.begin_shutdown().await;
        let started = Instant::now();
        let mut tasks = {
            let mut registry = self
                .lifecycle
                .tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            registry.drain().map(|(_, slot)| slot).collect::<Vec<_>>()
        };
        let mut deadline_breached = false;
        for slot in &mut tasks {
            match slot {
                WsConnectionSlot::Reserved { resources, .. } => {
                    let _ = resources.lock().unwrap_or_else(|p| p.into_inner()).take();
                }
                WsConnectionSlot::Installed(task) => {
                    let remaining = deadline.saturating_sub(started.elapsed());
                    let grace = remaining.min(Duration::from_secs(2));
                    match tokio::time::timeout(grace, &mut *task).await {
                        Ok(Ok(())) | Ok(Err(_)) => {}
                        Err(_) => {
                            deadline_breached = true;

                            // Cancellation is cooperative, but the server still owns the
                            // task. Force-reap a handler that ignores the signal before
                            // the truthful overall deadline expires; its OwnerLease guard
                            // then runs synchronously during task destruction.
                            task.abort();
                            let _ = task.await;
                        }
                    }
                }
            }
        }
        let mut request_slots = {
            let mut registry = self
                .lifecycle
                .request_tasks
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            registry.drain().map(|(_, slot)| slot).collect::<Vec<_>>()
        };
        for slot in &mut request_slots {
            match slot {
                WsRequestSlot::Reserved { permit, cancel } => {
                    let _ = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                    if let Some(cancel) = cancel.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = cancel.send(());
                    }
                }
                WsRequestSlot::Installed { task, cancel } => {
                    if let Some(cancel) = cancel.lock().unwrap_or_else(|p| p.into_inner()).take() {
                        let _ = cancel.send(());
                    }
                    let remaining = deadline
                        .saturating_sub(started.elapsed())
                        .min(Duration::from_secs(2));
                    if tokio::time::timeout(remaining, &mut *task).await.is_err() {
                        deadline_breached = true;
                        task.abort();
                        let _ = task.await;
                    }
                }
            }
        }
        self.reap().await;
        let connection_tasks = self.task_count();
        let request_tasks = self.request_task_count();
        if deadline_breached || connection_tasks != 0 || request_tasks != 0 {
            tracing::error!(?deadline, elapsed = ?started.elapsed(), connection_tasks, request_tasks, "WS shutdown exceeded its bounded cleanup lifecycle");
            Err("WS shutdown exceeded its deadline")
        } else {
            Ok(())
        }
    }

    pub async fn shutdown(&self) -> Result<(), &'static str> {
        self.drain(WS_SHUTDOWN_DEADLINE).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    #[serde(default)]
    pub kind: Option<String>, // ожидаем "hello"
    #[serde(rename = "protocolVersion", skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    #[serde(rename = "apiVersion", default)]
    pub api_version: Option<String>,
    pub token: Option<String>,
    pub pid: Option<u32>,
    #[serde(rename = "clientId", default)]
    pub client_id: Option<String>,
    #[serde(rename = "clientClass", default)]
    pub client_class: Option<String>,
    #[serde(rename = "clientVersion", default)]
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelloOkResponse<'a> {
    pub kind: &'static str, // "hello_ok"
    #[serde(rename = "apiVersion")]
    pub api_version: &'a str,
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
    Accept {
        compatibility: Compatibility,
        transport: TransportKind,
    },
    Reject {
        code: &'static str,
        message: String,
    },
}

/// Чистая функция (детерминированная) — отделена от network IO, тестируется легко.
pub fn validate_hello(hello: &HelloMessage, expected_token: &str) -> HelloOutcome {
    if hello.protocol_version.is_some() {
        return HelloOutcome::Reject {
            code: handshake_errors::UPGRADE_REQUIRED,
            message: "protocolVersion is no longer supported; upgrade to apiVersion".into(),
        };
    }
    let Some(raw_version) = hello.api_version.as_deref() else {
        return HelloOutcome::Reject {
            code: handshake_errors::MISSING_PROTOCOL_VERSION,
            message: "client must send apiVersion in hello".into(),
        };
    };
    if hello.kind.as_deref() != Some("hello") {
        return HelloOutcome::Reject {
            code: handshake_errors::MALFORMED_HELLO,
            message: "first frame kind must be hello".into(),
        };
    }

    let parsed_version = match ProtocolVersion::parse(raw_version) {
        Ok(v) => v,
        Err(e) => {
            return HelloOutcome::Reject {
                code: handshake_errors::MALFORMED_PROTOCOL_VERSION,
                message: format!("invalid apiVersion {raw_version:?}: {e}"),
            };
        }
    };

    let server_version = &API_VERSION_CURRENT;
    let compatibility = parsed_version.is_compatible_with_server(server_version);
    if matches!(compatibility, Compatibility::Incompatible) {
        return HelloOutcome::Reject {
            code: handshake_errors::INCOMPATIBLE_PROTOCOL_VERSION,
            message: format!(
                "client protocol MAJOR={} differs from server MAJOR={}",
                parsed_version.major, server_version.major
            ),
        };
    }

    let token = match &hello.token {
        Some(t) => t,
        None => {
            return HelloOutcome::Reject {
                code: handshake_errors::MISSING_TOKEN,
                message: "client must send auth token in hello".into(),
            };
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
            };
        }
    };

    match auth::validate_pid_belongs_to_current_user(pid) {
        Ok(()) => HelloOutcome::Accept {
            compatibility,
            transport: TransportKind::ApiV1,
        },
        Err(auth::AuthError::PidNotFound { .. }) => HelloOutcome::Reject {
            code: handshake_errors::INVALID_PID,
            message: "PID authorization failed".into(),
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
    dispatcher: crate::engine_dispatch::EngineDispatcher,
    ark_host: Arc<ArkHost>,
    auth_token: Arc<String>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    agents: Arc<tokio::sync::OnceCell<Arc<AgentsService>>>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    lifecycle: Arc<WsLifecycle>,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_operation(
    request: crate::engine_dispatch::DispatchRequest,
    ark_host: Arc<ArkHost>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    app_index: Arc<AppIndex>,
    file_index: Arc<FileIndex>,
    agents: Arc<tokio::sync::OnceCell<Arc<AgentsService>>>,
    agents_data_dir: Arc<std::path::PathBuf>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    rpc_diagnostics: SharedRpcDiagnostics,
    protocol_usage: Arc<ProtocolUsageStore>,
    package_service: Arc<PackageService>,
    store_catalog: Option<Arc<StoreCatalogService>>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    manager_state: ManagerState,
    correlation_id: Arc<String>,
    client_id: ClientId,
) -> Value {
    let operation = request.operation.as_str().to_owned();
    let req_id = request.request_id;
    let params = request.params;
    let started = std::time::Instant::now();
    let connection_id = client_id;
    #[cfg(test)]
    if operation == "test.stall" {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    let response = if operation.starts_with("package.snapshot.") || operation.starts_with("grant.")
    {
        if !request.client.desktop_authorized {
            LocalResponse::err("desktop authority denied")
        } else {
            let snapshots = snapshots;
            let owner = format!("desktop-connection-{connection_id}");
            match operation.as_str() {
                "package.snapshot.reserve" => {
                    let package_id = params.get("packageId").and_then(Value::as_str);
                    let source = params.get("source").and_then(Value::as_str);
                    match (package_id, source) {
                        (Some(package_id), Some(source)) => {
                            let build = package_service.clone();
                            let package_id = package_id.to_owned();
                            let source = source.to_owned();
                            let build_package_id = package_id.clone();
                            let build_source = source.clone();
                            match tokio::task::spawn_blocking(move || {
                                build.build_engine_snapshot(&build_package_id, &build_source)
                            })
                            .await
                            {
                                Ok(Ok(files)) => {
                                    let (root_realpath, root_dev, root_ino) = match package_service
                                        .engine_snapshot_identity(&package_id, &source)
                                    {
                                        Ok(value) => value,
                                        Err(error) => {
                                            return serde_json::json!({ "ok": false, "data": null, "error": error.to_string() });
                                        }
                                    };
                                    match serde_json::to_vec(&files) {
                                        Ok(bytes) => match snapshots.reserve(
                                            &owner,
                                            &package_id,
                                            &source,
                                            bytes,
                                        ) {
                                            Ok(handle) => match snapshots.size(&handle, &owner) {
                                                Ok(size) => LocalResponse::ok(serde_json::json!({
                                                    "handle": handle,
                                                    "packageId": package_id,
                                                    "source": source,
                                                    "size": size,
                                                    "rootRealpath": root_realpath,
                                                    "rootIdentity": { "dev": root_dev, "ino": root_ino },
                                                })),
                                                Err(error) => LocalResponse::err(error.to_string()),
                                            },
                                            Err(error) => LocalResponse::err(error.to_string()),
                                        },
                                        Err(error) => LocalResponse::err(error.to_string()),
                                    }
                                }
                                Ok(Err(error)) => LocalResponse::err(error.to_string()),
                                Err(error) => LocalResponse::err(error.to_string()),
                            }
                        }
                        _ => LocalResponse::err("package snapshot requires packageId and source"),
                    }
                }
                "grant.authority.register" => {
                    if !request.client.desktop_authorized {
                        return serde_json::json!({"ok": false, "data": null, "error": "grant authority denied"});
                    }
                    let Some(grant_owner) =
                        desktop_authority
                            .owner(connection_id)
                            .map(|(session_id, generation)| GrantOwner {
                                session_id,
                                generation,
                                connection_id,
                            })
                    else {
                        return serde_json::json!({"ok": false, "data": null, "error": "grant authority denied"});
                    };
                    let Some(extension_id) = params.get("extensionId").and_then(Value::as_str)
                    else {
                        return serde_json::json!({"ok": false, "data": null, "error": "invalid grant request"});
                    };
                    let Some(root) = params.get("root").and_then(Value::as_str) else {
                        return serde_json::json!({"ok": false, "data": null, "error": "invalid grant request"});
                    };
                    let Some(provenance) = params
                        .get("provenance")
                        .and_then(Value::as_str)
                        .and_then(GrantProvenance::parse)
                    else {
                        return serde_json::json!({"ok": false, "data": null, "error": "invalid grant request"});
                    };
                    let exact_file = params
                        .get("exactFile")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    match grants.register(
                        &grant_owner,
                        extension_id,
                        Path::new(root),
                        exact_file,
                        provenance,
                        None,
                    ) {
                        Ok((grant_id, _identity, persistent_id)) => LocalResponse::ok(
                            serde_json::json!({"grantId": grant_id, "persistentGrantId": persistent_id, "exactFile": exact_file, "provenance": provenance.as_str()}),
                        ),
                        Err(_) => LocalResponse::err("grant authority denied"),
                    }
                }
                "grant.authority.reopen" => {
                    let (Some(persistent_id), Some(extension_id)) = (
                        params.get("persistentGrantId").and_then(Value::as_str),
                        params.get("extensionId").and_then(Value::as_str),
                    ) else {
                        return serde_json::json!({"ok": false, "data": null, "error": "invalid grant request"});
                    };
                    let Some(grant_owner) =
                        desktop_authority
                            .owner(connection_id)
                            .map(|(session_id, generation)| GrantOwner {
                                session_id,
                                generation,
                                connection_id,
                            })
                    else {
                        return serde_json::json!({"ok": false, "data": null, "error": "grant authority denied"});
                    };
                    match grants.reopen(&grant_owner, persistent_id, extension_id) {
                        Ok((grant_id, _)) => {
                            LocalResponse::ok(serde_json::json!({"grantId": grant_id}))
                        }
                        Err(_) => LocalResponse::err("grant authority denied"),
                    }
                }
                "grant.snapshot.reserve" => {
                    if params.get("root").is_some()
                        || params.get("path").is_some()
                        || params.get("identityDev").is_some()
                        || params.get("identityIno").is_some()
                        || params.get("exactFile").is_some()
                    {
                        return serde_json::json!({ "ok": false, "data": null, "error": "grant authority denied" });
                    }
                    let grant_id = params.get("grantId").and_then(Value::as_str);
                    if grant_id.is_none() {
                        return serde_json::json!({ "ok": false, "data": null, "error": "grant authority denied" });
                    }
                    let (Some(grant_id), Some(extension_id), Some(relative)) = (
                        params.get("grantId").and_then(Value::as_str),
                        params.get("extensionId").and_then(Value::as_str),
                        params.get("relativeAsset").and_then(Value::as_str),
                    ) else {
                        return serde_json::json!({"ok": false, "data": null, "error": "invalid grant request"});
                    };
                    let Some(grant_owner) =
                        desktop_authority
                            .owner(connection_id)
                            .map(|(session_id, generation)| GrantOwner {
                                session_id,
                                generation,
                                connection_id,
                            })
                    else {
                        return serde_json::json!({"ok": false, "data": null, "error": "grant authority denied"});
                    };
                    let requested: Vec<&str> = relative
                        .split('/')
                        .filter(|part| !part.is_empty())
                        .collect();
                    if requested.is_empty()
                        || requested.iter().any(|part| *part == "." || *part == "..")
                    {
                        return serde_json::json!({"ok": false, "data": null, "error": "grant snapshot denied"});
                    }
                    match grants.read(
                        grant_id,
                        &grant_owner,
                        extension_id,
                        &requested,
                        16 * 1024 * 1024,
                    ) {
                        Ok(bytes) => {
                            match snapshots.reserve(&owner, extension_id, "grant", bytes) {
                                Ok(handle) => {
                                    LocalResponse::ok(serde_json::json!({"handle": handle}))
                                }
                                Err(_) => LocalResponse::err("grant snapshot denied"),
                            }
                        }
                        Err(_) => LocalResponse::err("grant snapshot denied"),
                    }
                }
                "package.snapshot.chunk" | "grant.snapshot.chunk" => {
                    let handle = params.get("handle").and_then(Value::as_str);
                    let offset = params
                        .get("offset")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize);
                    let length = params
                        .get("length")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize);
                    match (handle, offset, length) {
                        (Some(handle), Some(offset), Some(length)) => {
                            match snapshots.chunk(handle, &owner, offset, length) {
                                Ok(bytes) => LocalResponse::ok(serde_json::json!({
                                    "handle": handle,
                                    "offset": offset,
                                    "bytes": base64::engine::general_purpose::STANDARD.encode(bytes),
                                })),
                                Err(error) => LocalResponse::err(error.to_string()),
                            }
                        }
                        _ => LocalResponse::err("invalid package snapshot chunk"),
                    }
                }
                "package.snapshot.close" | "grant.snapshot.close" => {
                    match params.get("handle").and_then(Value::as_str) {
                        Some(handle) => match snapshots.close(handle, &owner) {
                            Ok(()) => LocalResponse::ok(serde_json::json!({ "closed": true })),
                            Err(error) => LocalResponse::err(error.to_string()),
                        },
                        None => LocalResponse::err("invalid package snapshot handle"),
                    }
                }
                _ => LocalResponse::err("unknown package snapshot operation"),
            }
        }
    } else if operation == "diagnostics.snapshot" {
        LocalResponse::ok(
            build_diagnostics_snapshot(
                &ark_host,
                &rpc_diagnostics,
                &file_index,
                &usage_diagnostics,
                &app_index,
                &protocol_usage,
                &package_service,
                &correlation_id,
                &agents_data_dir,
            )
            .await,
        )
    } else if let Some(rest) = operation.strip_prefix("agents.") {
        match agents
            .get_or_try_init(|| async {
                AgentsService::new_with_events(&agents_data_dir, agent_events.clone())
            })
            .await
        {
            Ok(service) => service
                .handle(rest, params)
                .await
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            Err(error) => LocalResponse::err(error.clone()),
        }
    } else if let Some(rest) = operation.strip_prefix("dictation.") {
        {
            let result = handle_dictation_op(rest, params, &dictation_host).await;
            LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            }
        }
    } else if let Some(rest) = operation.strip_prefix("pomodoro.") {
        {
            let result = handle_pomodoro_op(rest, params, &pomodoro_host).await;
            LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            }
        }
    } else if let Some(rest) = operation.strip_prefix("export.") {
        handle_export_op(rest, params, &ark_host).await
    } else if let Some(rest) = operation.strip_prefix("arrancador.") {
        handle_arrancador_op(rest, params, &ark_host).await
    } else if let Some(rest) = operation.strip_prefix("focus.") {
        {
            let result = handle_focus_op(rest, params, &ark_host).await;
            LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            }
        }
    } else if let Some(rest) = operation.strip_prefix("app_index.") {
        handle_app_index_op(rest, params, &app_index).await
    } else if let Some(rest) = operation.strip_prefix("calculator.") {
        handle_calculator_op(rest, params, &agents_data_dir).await
    } else if let Some(rest) = operation.strip_prefix("integrations.") {
        integrations::handle_operation(rest, params, &ark_host, &agents_data_dir)
            .await
            .map(LocalResponse::ok)
            .unwrap_or_else(LocalResponse::err)
    } else if let Some(rest) = operation.strip_prefix("file_index.") {
        handle_file_index_op(rest, params, &file_index).await
    } else if let Some(rest) = operation.strip_prefix("commands.") {
        handle_command_op(rest, params, &command_bus, connection_id).await
    } else if let Some(rest) = operation.strip_prefix("store.") {
        handle_store_op(rest, params, &package_service, store_catalog.as_deref()).await
    } else if let Some(rest) = operation.strip_prefix("packages.") {
        handle_package_op(rest, params, &package_service).await
    } else if matches!(
        operation.as_str(),
        "engine.settings.get"
            | "engine.settings.set"
            | "engine.autostart.get"
            | "engine.autostart.set"
    ) {
        match operation.as_str() {
            "engine.settings.get" => manager_state
                .settings()
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            "engine.settings.set" => manager_state
                .set_settings_patch(
                    params
                        .get("warm_timeout_seconds")
                        .and_then(Value::as_u64)
                        .or_else(|| {
                            params
                                .pointer("/desktop_host/warm_timeout_seconds")
                                .and_then(Value::as_u64)
                        }),
                    params
                        .get("usage_tracker_enabled")
                        .and_then(Value::as_bool)
                        .or_else(|| {
                            params
                                .pointer("/usage_tracker/enabled")
                                .and_then(Value::as_bool)
                        }),
                )
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            "engine.autostart.get" => LocalResponse::ok(manager_state.autostart()),
            "engine.autostart.set" => manager_state
                .set_autostart(
                    params
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .map(LocalResponse::ok)
                .unwrap_or_else(LocalResponse::err),
            _ => unreachable!(),
        }
    } else if let Some(rest) = operation.strip_prefix("manager.") {
        let result = match rest {
            "data.summary" => manager_state
                .data_summary(&ark_host, package_service.storage_root())
                .await
                .map(LocalResponse::ok),
            "data.types" => manager_state
                .data_types(&ark_host)
                .await
                .map(LocalResponse::ok),
            "data.list" => manager_state
                .data_list(&ark_host, &params)
                .await
                .map(LocalResponse::ok),
            "data.search" => manager_state
                .data_search(&ark_host, &params)
                .await
                .map(LocalResponse::ok),
            "diagnostics.snapshot" => Ok(LocalResponse::ok(
                manager_state
                    .diagnostics_snapshot(
                        &rpc_diagnostics,
                        &usage_diagnostics,
                        &protocol_usage,
                        &package_service,
                    )
                    .await,
            )),
            "diagnostics.log_tail" => Ok(LocalResponse::ok(
                manager_state.log_tail(&rpc_diagnostics).await,
            )),
            "diagnostics.support_bundle.create" => manager_state
                .create_bundle(
                    manager_state
                        .diagnostics_snapshot(
                            &rpc_diagnostics,
                            &usage_diagnostics,
                            &protocol_usage,
                            &package_service,
                        )
                        .await,
                    manager_state.log_tail(&rpc_diagnostics).await,
                )
                .await
                .map(LocalResponse::ok),
            "diagnostics.support_bundle.save" => manager_state
                .save_bundle(
                    params
                        .get("handle")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    params
                        .get("destination")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .await
                .map(LocalResponse::ok),
            "diagnostics.support_bundle.cancel" => manager_state
                .cancel_bundle(
                    params
                        .get("handle")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .await
                .map(LocalResponse::ok),
            _ => Err(format!("manager.{rest}: unknown-operation")),
        };
        result.unwrap_or_else(LocalResponse::err)
    } else {
        match ark_host.request(&operation, params).await {
            Ok(result) => LocalResponse {
                ok: result.ok,
                data: result.data,
                error: result.error,
            },
            Err(error) => LocalResponse::err(format!("ark_host: {error}")),
        }
    };
    let mut envelope = serde_json::Map::new();
    if let Some(id) = req_id {
        envelope.insert("id".into(), Value::String(id));
    }
    envelope.insert("ok".into(), Value::Bool(response.ok));
    envelope.insert("data".into(), response.data);
    if let Some(error) = response.error {
        envelope.insert("error".into(), Value::String(error));
    }
    let payload = Value::Object(envelope);
    observe_rpc_payload(&rpc_diagnostics, &operation, started, &payload.to_string());
    payload
}

#[allow(clippy::too_many_arguments)]
fn make_dispatcher(
    ark_host: Arc<ArkHost>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    app_index: Arc<AppIndex>,
    file_index: Arc<FileIndex>,
    agents: Arc<tokio::sync::OnceCell<Arc<AgentsService>>>,
    agents_data_dir: Arc<std::path::PathBuf>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    rpc_diagnostics: SharedRpcDiagnostics,
    protocol_usage: Arc<ProtocolUsageStore>,
    package_service: Arc<PackageService>,
    store_catalog: Option<Arc<StoreCatalogService>>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    manager_state: ManagerState,
    correlation_id: Arc<String>,
) -> crate::engine_dispatch::EngineDispatcher {
    let cleanup_bus = command_bus.clone();
    let handler: crate::engine_dispatch::DispatchHandler =
        Arc::new(move |request: crate::engine_dispatch::DispatchRequest| {
            let ark_host = ark_host.clone();
            let command_bus = command_bus.clone();
            let pomodoro_host = pomodoro_host.clone();
            let dictation_host = dictation_host.clone();
            let app_index = app_index.clone();
            let file_index = file_index.clone();
            let agents = agents.clone();
            let agents_data_dir = agents_data_dir.clone();
            let agent_events = agent_events.clone();
            let usage_diagnostics = usage_diagnostics.clone();
            let rpc_diagnostics = rpc_diagnostics.clone();
            let protocol_usage = protocol_usage.clone();
            let package_service = package_service.clone();
            let store_catalog = store_catalog.clone();
            let snapshots = snapshots.clone();
            let grants = grants.clone();
            let desktop_authority = desktop_authority.clone();
            let manager_state = manager_state.clone();
            let correlation_id = correlation_id.clone();
            let connection_id = request.client.connection_id.unwrap_or(0);
            Box::pin(async move {
                Ok(dispatch_operation(
                    request,
                    ark_host,
                    command_bus,
                    pomodoro_host,
                    dictation_host,
                    app_index,
                    file_index,
                    agents,
                    agents_data_dir,
                    agent_events,
                    usage_diagnostics,
                    rpc_diagnostics,
                    protocol_usage,
                    package_service,
                    store_catalog,
                    snapshots,
                    grants,
                    desktop_authority,
                    manager_state,
                    correlation_id,
                    connection_id,
                )
                .await)
            })
        });
    let cleanup: crate::engine_dispatch::CleanupHandler = Arc::new(move |client_id| {
        if let Some(snapshot) = cleanup_bus.unregister_all_sync(client_id) {
            cleanup_bus.broadcast_changed_snapshot(snapshot);
        }
    });
    crate::engine_dispatch::EngineDispatcher::with_cleanup(handler, cleanup)
}

struct WsConnectionOwnerGuard {
    dispatcher: crate::engine_dispatch::EngineDispatcher,
    lease: Option<crate::engine_dispatch::OwnerLease>,
}

impl WsConnectionOwnerGuard {
    fn new(
        dispatcher: crate::engine_dispatch::EngineDispatcher,
        lease: crate::engine_dispatch::OwnerLease,
    ) -> Self {
        Self {
            dispatcher,
            lease: Some(lease),
        }
    }

    fn id(&self) -> u64 {
        self.lease.as_ref().expect("owner guard lease").id()
    }
}

impl Drop for WsConnectionOwnerGuard {
    fn drop(&mut self) {
        let Some(lease) = self.lease.take() else {
            return;
        };
        let dispatcher = self.dispatcher.clone();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            dispatcher.cleanup_connection_sync(&lease);
        }));
        lease.release();
    }
}

impl WsServer {
    /// Биндит TcpListener на 127.0.0.1 + случайный свободный порт.
    /// `data_dir` — куда писать persisted pomodoro state.
    #[allow(clippy::too_many_arguments)]
    pub async fn bind(
        ark_host: Arc<ArkHost>,
        auth_token: String,
        data_dir: std::path::PathBuf,
        app_index: Arc<AppIndex>,
        file_index: Arc<FileIndex>,
        usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
        protocol_usage: Arc<ProtocolUsageStore>,
        package_service: Arc<PackageService>,
        correlation_id: String,
    ) -> Result<Self, WsServerError> {
        let addr: SocketAddr = "127.0.0.1:0"
            .parse()
            .expect("hardcoded socket literal is always valid");
        let listener = TcpListener::bind(addr).await?;
        let (agent_events, _) = tokio::sync::broadcast::channel(512);
        let command_bus = Arc::new(CommandBus::new());
        let pomodoro_host = PomodoroHost::new(data_dir.clone());
        let dictation_host = DictationHost::new();
        let agents = Arc::new(tokio::sync::OnceCell::new());
        let agents_data_dir = Arc::new(data_dir.clone());
        let rpc_diagnostics: SharedRpcDiagnostics = Arc::new(RpcDiagnostics::new());
        let manager_state = ManagerState::new(data_dir.clone());
        let store_catalog = StoreCatalogService::open_compiled(&data_dir)
            .ok()
            .map(Arc::new);
        let snapshots = Arc::new(crate::package_worker_broker::SnapshotRegistry::new());
        let grants = Arc::new(GrantAuthorityRegistry::with_data_dir(data_dir.clone()));
        let desktop_authority = Arc::new(crate::desktop_authority::DesktopAuthorityRegistry::new());
        let dispatcher = make_dispatcher(
            ark_host.clone(),
            command_bus.clone(),
            pomodoro_host.clone(),
            dictation_host.clone(),
            app_index.clone(),
            file_index.clone(),
            agents.clone(),
            agents_data_dir.clone(),
            agent_events.clone(),
            usage_diagnostics.clone(),
            rpc_diagnostics.clone(),
            protocol_usage.clone(),
            package_service.clone(),
            store_catalog,
            snapshots.clone(),
            grants.clone(),
            desktop_authority.clone(),
            manager_state.clone(),
            Arc::new(correlation_id.clone()),
        );
        let lifecycle = Arc::new(WsLifecycle::default());
        Ok(WsServer {
            listener,
            dispatcher,
            ark_host,
            auth_token: Arc::new(auth_token),
            command_bus,
            pomodoro_host,
            dictation_host,
            agents,
            agent_events,
            protocol_usage,
            correlation_id: Arc::new(correlation_id),
            lifecycle,
            desktop_authority,
            snapshots,
            grants,
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "public API returns the local server error enum"
    )]
    pub fn local_addr(&self) -> Result<SocketAddr, WsServerError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or_default()
    }

    pub fn agents_handle(&self) -> Arc<tokio::sync::OnceCell<Arc<AgentsService>>> {
        self.agents.clone()
    }

    pub fn dispatcher(&self) -> crate::engine_dispatch::EngineDispatcher {
        self.dispatcher.clone()
    }

    pub fn registration_count(&self) -> usize {
        self.command_bus.registration_count_sync()
    }

    pub fn command_bus_handle(&self) -> Arc<CommandBus> {
        self.command_bus.clone()
    }

    pub fn desktop_authority(&self) -> Arc<crate::desktop_authority::DesktopAuthorityRegistry> {
        self.desktop_authority.clone()
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    pub fn grant_count(&self) -> usize {
        self.grants.len()
    }

    pub fn snapshots_handle(&self) -> Arc<crate::package_worker_broker::SnapshotRegistry> {
        self.snapshots.clone()
    }

    pub fn grants_handle(&self) -> Arc<GrantAuthorityRegistry> {
        self.grants.clone()
    }

    pub fn shutdown_handle(&self) -> WsShutdownHandle {
        WsShutdownHandle {
            lifecycle: self.lifecycle.clone(),
        }
    }

    /// The accept loop is itself joined by the runtime owner; every connection
    /// task is inserted into the server registry before admission is released.
    pub async fn run(self) -> Result<(), WsServerError> {
        let shutdown = self.shutdown_handle();
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                accepted = self.listener.accept() => {
                    let (stream, _peer) = accepted?;
                    let permit = match self.lifecycle.capacity.clone().try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            let mut stream = stream;
                            let _ = stream.shutdown().await;
                            continue;
                        }
                    };
                    let _admission = shutdown.lifecycle.admission.lock().unwrap_or_else(|p| p.into_inner());
                    if self.lifecycle.closed.load(Ordering::Acquire) {
                        drop(permit);
                        drop(_admission);
                        let mut stream = stream;
                        let _ = stream.shutdown().await;
                        continue;
                    }
                    let client_id = match self.dispatcher.allocate_owner() {
                        Ok(owner) => owner,
                        Err(error) => {
                            eprintln!("[kepler.ws] owner allocation failed: {error}");
                            drop(permit);
                            continue;
                        }
                    };
                    let task_id = self.lifecycle.next_task.fetch_add(1, Ordering::Relaxed);
                    let ark_host = self.ark_host.clone();
                    let token = self.auth_token.clone();
                    let bus = self.command_bus.clone();
                    let pomo = self.pomodoro_host.clone();
                    let dict = self.dictation_host.clone();
                    let agent_events = self.agent_events.clone();
                    let protocol_usage = self.protocol_usage.clone();
                    let correlation_id = self.correlation_id.clone();
                    let dispatcher = self.dispatcher.clone();
                    let desktop_authority = self.desktop_authority.clone();
                    let snapshots = self.snapshots.clone();
                    let grants = self.grants.clone();
                    let task_shutdown = shutdown.clone();
                    let resources = Arc::new(Mutex::new(Some(WsConnectionResources {
                        stream: Some(stream),
                        permit: Some(permit),
                        owner_lease: Some(client_id),
                    })));
                    let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
                    self.lifecycle
                        .tasks
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(
                            task_id,
                            WsConnectionSlot::Reserved {
                                resources: resources.clone(),
                                start: start_sender,
                            },
                        );
                    let task_resources = resources.clone();
                    let task = tokio::spawn(async move {
                        if start_receiver.await.is_err() {
                            return;
                        }
                        let Some(mut resources) = task_resources
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .take()
                        else {
                            return;
                        };
                        let Some(stream) = resources.stream.take() else {
                            return;
                        };
                        let Some(owner_lease) = resources.owner_lease.take() else {
                            return;
                        };
                        let _permit = resources.permit.take();
                        // The task cannot execute any connection code until its
                        // JoinHandle has replaced the Reserved slot below.
                        if let Err(e) = handle_connection(
                            stream, ark_host, token, bus, pomo, dict, agent_events,
                            protocol_usage, correlation_id, dispatcher, owner_lease,
                            desktop_authority, snapshots, grants, task_shutdown,
                        ).await {
                            eprintln!("[kepler.ws] connection error: {e}");
                        }
                    });
                    let old = self
                        .lifecycle
                        .tasks
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(task_id, WsConnectionSlot::Installed(task));
                    debug_assert!(matches!(old, Some(WsConnectionSlot::Reserved { .. })));
                    if let Some(WsConnectionSlot::Reserved { start, .. }) = old {
                        let _ = start.send(());
                    }
                }
            }
            shutdown.reap().await;
        }
        shutdown.reap().await;
        Ok(())
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "connection task wires existing shared services"
)]
async fn handle_connection(
    stream: tokio::net::TcpStream,
    ark_host: Arc<ArkHost>,
    expected_token: Arc<String>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    protocol_usage: Arc<ProtocolUsageStore>,
    correlation_id: Arc<String>,
    dispatcher: crate::engine_dispatch::EngineDispatcher,
    owner_lease: crate::engine_dispatch::OwnerLease,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    shutdown: WsShutdownHandle,
) -> Result<(), WsServerError> {
    let owner_guard = WsConnectionOwnerGuard::new(dispatcher.clone(), owner_lease);
    let client_id = owner_guard.id();
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(MAX_WS_MESSAGE_BYTES),
        max_frame_size: Some(MAX_WS_MESSAGE_BYTES),
        ..Default::default()
    };
    let ws = tokio::select! {
        _ = shutdown.cancelled() => return Ok(()),
        result = tokio_tungstenite::accept_async_with_config(stream, Some(ws_config)) => result?,
    };
    let (mut sink, mut stream) = ws.split();

    // 1. Hello.
    let hello_msg = match tokio::select! {
        _ = shutdown.cancelled() => return Ok(()),
        frame = stream.next() => frame,
    } {
        Some(Ok(Message::Text(text))) => text,
        Some(Ok(Message::Binary(_))) | Some(Ok(_)) => {
            send_hello_error(
                &mut sink,
                handshake_errors::MALFORMED_HELLO,
                "first frame must be text JSON hello",
                &shutdown,
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
                &shutdown,
            )
            .await?;
            return Ok(());
        }
    };

    match validate_hello(&hello, &expected_token) {
        HelloOutcome::Reject { code, message } => {
            send_hello_error(&mut sink, code, &message, &shutdown).await?;
            return Ok(());
        }
        HelloOutcome::Accept {
            compatibility,
            transport,
        } => {
            if let Err(error) = protocol_usage.record(
                transport,
                hello.client_class.as_deref(),
                hello.client_version.as_deref(),
            ) {
                tracing::warn!(error = %error, "protocol usage persistence failed");
            }
            let response = HelloOkResponse {
                kind: "hello_ok",
                api_version: API_VERSION,
                compatibility: compatibility_label(&compatibility),
            };
            let payload = serde_json::to_string(&response)?;
            send_message(&mut sink, Message::Text(payload), &shutdown).await?;
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
    let mut dict_rx = dictation_host.subscribe();
    // Forward ark-core events (object_upserted/object_deleted/entity_changed/peer_*
    // и т.п.) — до 2026-05-20 это broadcast channel был не подключён к WS,
    // events не доходили до клиентов. Cross-app live updates (Eden subscribed
    // на object_upserted для taskRef) полагаются на этот forward.
    let mut ark_evt_rx = ark_host.subscribe_events();
    let mut agents_rx = agent_events.subscribe();

    loop {
        tokio::select! {
            // 2a. Outgoing: events from command_bus → client.
            evt = bus_rx.recv() => {
                match evt {
                    Ok(CommandBusEvent::Changed(list)) => {
                        let payload = serde_json::json!({
                            "event": "commands_changed",
                            "commands": list,
                        });
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Ok(CommandBusEvent::Invoked { id, params }) => {
                        let payload = serde_json::json!({
                            "event": "command_invoked",
                            "id": id,
                            "params": params,
                        });
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
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
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // Daedalus events are already flat wire envelopes.
            agent_evt = agents_rx.recv() => {
                match agent_evt {
                    Ok(payload) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2a'. Pomodoro events → forward as wire-formatted JSON.
            pevt = pomo_rx.recv() => {
                match pevt {
                    Ok(payload) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2a''. Dictation events (state_changed / transcript / config_changed) → forward.
            devt = dict_rx.recv() => {
                match devt {
                    Ok(payload) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
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
                        if send_message(&mut sink, Message::Pong(p), &shutdown).await.is_err() {
                            break;
                        }
                        continue;
                    }
                    Ok(_) => continue, // binary/pong — игнор
                    Err(error) => {
                        tracing::warn!(client_id, error = %error, "Engine WebSocket receive failed");
                        break;
                    }
                };

                let request_value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(error) => {
                        let malformed = Message::Text(serde_json::json!({
                            "ok": false,
                            "error": format!("malformed JSON: {error}"),
                        }).to_string());
                        if send_message(&mut sink, malformed, &shutdown).await.is_err() {
                            break;
                        }
                        continue;
                    }
                };
                if request_value.get("operation").and_then(Value::as_str)
                    == Some("desktop.authority.bind")
                {
                    let params = request_value.get("params").unwrap_or(&Value::Null);
                    let denied = params.get("root").is_some()
                        || params.get("path").is_some()
                        || params.get("sourceRoot").is_some();
                    let bind_result = (!denied).then(|| {
                        desktop_authority.bind(
                            params.get("sessionId").and_then(Value::as_str).unwrap_or_default(),
                            params.get("generation").and_then(Value::as_u64).unwrap_or_default(),
                            hello.pid.unwrap_or_default(),
                            params.get("credential").and_then(Value::as_str).unwrap_or_default(),
                            client_id,
                        )
                    });
                    let bound = bind_result.as_ref().is_some_and(Result::is_ok);

                    let response = serde_json::json!({
                        "id": request_value.get("id").cloned().unwrap_or(Value::Null),
                        "ok": bound,
                        "data": if bound { serde_json::json!({ "ok": true }) } else { Value::Null },
                        "code": if bound { Value::Null } else { Value::String("DESKTOP_AUTHORITY_BIND_DENIED".into()) },
                        "error": if bound { Value::Null } else { Value::String("desktop authority denied".into()) },
                    });
                    if send_message(&mut sink, Message::Text(response.to_string()), &shutdown).await.is_err() {
                        break;
                    }
                    continue;
                }
                let request = match crate::engine_dispatch::DispatchRequest::from_wire(request_value) {
                    Ok(request) => request.with_client(crate::engine_dispatch::DispatchClient {
                        pid: hello.pid,
                        class: hello.client_class.clone(),
                        version: hello.client_version.clone(),
                        correlation_id: Some(correlation_id.as_ref().clone()),
                        connection_id: Some(client_id),
                        desktop_authorized: desktop_authority.authorize(client_id),
                    }),
                    Err(error) => {
                        let invalid = Message::Text(serde_json::json!({
                            "id": serde_json::Value::Null,
                            "ok": false,
                            "error": error.to_string(),
                        }).to_string());
                        if send_message(&mut sink, invalid, &shutdown).await.is_err() {
                            break;
                        }
                        continue;
                    }
                };
                let request_id_value = request.request_id.clone();
                let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
                let (cancel_sender, cancel_receiver) = tokio::sync::oneshot::channel();
                let cancel = Arc::new(Mutex::new(Some(cancel_sender)));
                let permit = match shutdown.lifecycle.request_capacity.clone().try_acquire_owned() {
                    Ok(permit) => Arc::new(Mutex::new(Some(permit))),
                    Err(_) => {
                        let payload = serde_json::json!({"id": request_id_value, "ok": false, "error": "WS request capacity exhausted"});
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() { break; }
                        continue;
                    }
                };
                let request_id = shutdown.lifecycle.next_request.fetch_add(1, Ordering::Relaxed);
                let task_shutdown = shutdown.clone();
                let task_dispatcher = dispatcher.clone();
                let task = shutdown.install_request(request_id, permit.clone(), cancel.clone(), move |start_receiver| {
                    tokio::spawn(async move {
                        if start_receiver.await.is_err() {
                            return;
                        }
                        let _permit = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                        let dispatch = task_dispatcher.dispatch(request);
                        let result = tokio::select! {
                            _ = task_shutdown.cancelled() => Err("server shutting down".to_string()),
                            _ = cancel_receiver => Err("request cancelled".to_string()),
                            result = tokio::time::timeout(task_shutdown.response_deadline(), dispatch) => match result {
                                Ok(Ok(value)) => Ok(value),
                                Ok(Err(error)) => Err(error.to_string()),
                                Err(_) => Err("dispatch timed out".to_string()),
                            },
                        };
                        let _ = result_sender.send(result);
                    })
                });
                if !task {
                    break;
                }
                let mut result_receiver = result_receiver;
                let payload = loop {
                    tokio::select! {
                        result = &mut result_receiver => {
                            break match result {
                                Ok(Ok(payload)) => payload,
                                Ok(Err(error)) => serde_json::json!({"id": request_id_value, "ok": false, "error": error}),
                                Err(_) => serde_json::json!({"id": request_id_value, "ok": false, "error": "dispatch task failed"}),
                            };
                        }
                        _ = shutdown.cancelled() => {
                            shutdown.finish_request(request_id, true).await;
                            return Ok(());
                        }
                        frame = stream.next() => {
                            match frame {
                                Some(Ok(Message::Close(_))) | None => {
                                    shutdown.finish_request(request_id, true).await;
                                    return Ok(());
                                }
                                Some(Err(error)) => {
                                    tracing::warn!(client_id, error = %error, "Engine WebSocket receive failed during request");
                                    shutdown.finish_request(request_id, true).await;
                                    return Ok(());
                                }
                                Some(Ok(Message::Ping(p))) => {
                                    if send_message(&mut sink, Message::Pong(p), &shutdown).await.is_err() {
                                        shutdown.finish_request(request_id, true).await;
                                        return Ok(());
                                    }
                                }
                                Some(Ok(Message::Text(_)))
                                | Some(Ok(Message::Binary(_)))
                                | Some(Ok(Message::Pong(_)))
                                | Some(Ok(Message::Frame(_))) => {
                                    let busy = serde_json::json!({"id": serde_json::Value::Null, "ok": false, "error": "WS request busy"});
                                    if send_message(&mut sink, Message::Text(busy.to_string()), &shutdown).await.is_err() {
                                        shutdown.finish_request(request_id, true).await;
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                };
                shutdown.finish_request(request_id, false).await;
                if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                    break;
                }
            }
        }
    }

    let grant_owner = desktop_authority
        .owner(client_id)
        .map(|(session_id, generation)| GrantOwner {
            session_id,
            generation,
            connection_id: client_id,
        });
    snapshots.close_owner(&format!("desktop-connection-{client_id}"));
    if let Some(owner) = grant_owner {
        grants.close_owner(owner);
    }
    desktop_authority.disconnect(client_id);
    Ok(())
}

// Локальный response от `commands.*` обработчика. Та же форма что у
// `ArkResponse`, но конструируется без обращения к ark-core-rpc.
include!("ws_server_tests.rs");
