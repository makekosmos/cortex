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
            }
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

#[allow(clippy::too_many_arguments)]
async fn build_diagnostics_snapshot(
    ark_host: &Arc<ArkHost>,
    rpc_diagnostics: &SharedRpcDiagnostics,
    file_index: &Arc<FileIndex>,
    usage_diagnostics: &Arc<UsageTrackerDiagnosticsState>,
    app_index: &Arc<AppIndex>,
    protocol_usage: &Arc<ProtocolUsageStore>,
    package_service: &Arc<PackageService>,
    correlation_id: &str,
    data_dir: &std::path::Path,
) -> serde_json::Value {
    let app_index_snapshot = app_index.diagnostics_snapshot().await;
    let app_index_background_worker = app_index_snapshot.background_worker.clone();
    serde_json::json!({
        "rpc": rpc_diagnostics.snapshot(),
        "file_index": file_index.diagnostics_snapshot(),
        "usage_tracker": usage_diagnostics.snapshot(),
        "app_index": app_index_snapshot,
          "protocol_usage": protocol_usage.snapshot(),
          "package_workers": package_service.worker_diagnostics(),
        "correlation_id": correlation_id,
        "ark_core": {
            "stderr_tail": ark_host.stderr_tail_snapshot(),
        },
        "engine_supervisor": crate::engine_supervisor::diagnostics_snapshot(data_dir),
        "background_workers": {
            "app_index": app_index_background_worker,
            "db_backup": crate::db_backup::diagnostics_snapshot(),
        },
    })
}

fn observe_rpc_payload(
    rpc_diagnostics: &SharedRpcDiagnostics,
    operation: &str,
    started: std::time::Instant,
    payload: &str,
) {
    rpc_diagnostics.observe_response(operation, started.elapsed(), payload.len());
}

async fn handle_package_op(
    subop: &str,
    params: serde_json::Value,
    service: &Arc<PackageService>,
) -> LocalResponse {
    match subop {
        "list" => {
            let kind = match params.get("kind").and_then(Value::as_str) {
                None => None,
                Some("app") => Some(crate::package_service::PackageKind::App),
                Some("source") => Some(crate::package_service::PackageKind::Source),
                Some("bridge") => Some(crate::package_service::PackageKind::Bridge),
                Some(_) => return LocalResponse::err("packages.list: invalid-kind"),
            };
            let catalog_kind = kind.clone();
            let installed = package_blocking({
                let service = service.clone();
                move || service.list_filtered(kind)
            })
            .await;
            match installed {
                Ok(list) => {
                    let catalog = service
                        .catalog_packages(catalog_kind.as_ref())
                        .unwrap_or_default();
                    let mut value =
                        serde_json::to_value(&list).unwrap_or_else(|_| serde_json::json!({}));
                    value["catalog"] = serde_json::json!(catalog);
                    LocalResponse::ok(value)
                }
                Err(error) => package_response::<crate::package_service::PackageListSummary>(
                    subop,
                    Err(error),
                ),
            }
        }
        "trust_status" => LocalResponse::ok(serde_json::json!({
            "trust": service.trust_summary(),
            "catalog": service.catalog_summary(),
        })),
        "refresh_catalog" => package_response(subop, service.refresh_catalog().await),
        "catalog_apply" => {
            let Some(document) = params.get("document").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.catalog_apply: invalid-request");
            };
            let signatures = match package_signatures(&params) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let document = document.as_bytes().to_vec();
            package_response(
                subop,
                package_blocking({
                    let service = service.clone();
                    move || service.apply_catalog(document, signatures)
                })
                .await,
            )
        }
        "transition_apply" => {
            let Some(document) = params.get("document").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.transition_apply: invalid-request");
            };
            let signatures = match package_signatures(&params) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let document = document.as_bytes().to_vec();
            package_response(
                subop,
                package_blocking({
                    let service = service.clone();
                    move || service.apply_transition(&document, signatures)
                })
                .await
                .map(|()| serde_json::json!({ "applied": true })),
            )
        }
        "revocation_apply" => {
            let Some(document) = params.get("document").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.revocation_apply: invalid-request");
            };
            let signatures = match package_signatures(&params) {
                Ok(value) => value,
                Err(response) => return response,
            };
            package_response(
                subop,
                service
                    .apply_revocations_with_worker_stop(document.as_bytes(), signatures)
                    .await
                    .map(|()| serde_json::json!({ "applied": true })),
            )
        }
        "install" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.install: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.install: invalid-request");
            };
            let id = id.to_owned();
            let version = version.to_owned();
            if let Some(archive_path) = params
                .get("archive_path")
                .and_then(serde_json::Value::as_str)
            {
                let archive_path = archive_path.to_owned();
                package_response(
                    subop,
                    service
                        .install_from_path_with_worker_stop(&id, &version, archive_path)
                        .await,
                )
            } else {
                package_response(subop, service.install_from_catalog(&id, &version).await)
            }
        }
        "set_enabled" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.set_enabled: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.set_enabled: invalid-request");
            };
            let Some(enabled) = params.get("enabled").and_then(serde_json::Value::as_bool) else {
                return LocalResponse::err("packages.set_enabled: invalid-request");
            };
            package_response(subop, service.set_enabled(id, version, enabled).await)
        }
        "bridge_config" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.bridge_config: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.bridge_config: invalid-request");
            };
            package_response(subop, service.bridge_config(id, version))
        }
        "bridge_config_set" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.bridge_config_set: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.bridge_config_set: invalid-request");
            };
            let Some(config) = params
                .get("config")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
            else {
                return LocalResponse::err("packages.bridge_config_set: invalid-request");
            };
            package_response(subop, service.set_bridge_config(id, version, config).await)
        }
        "uninstall" => {
            let Some(id) = params
                .get("id")
                .or_else(|| params.get("package_id"))
                .and_then(serde_json::Value::as_str)
            else {
                return LocalResponse::err("packages.uninstall: invalid-request");
            };
            let Some(version) = params.get("version").and_then(serde_json::Value::as_str) else {
                return LocalResponse::err("packages.uninstall: invalid-request");
            };
            package_response(
                subop,
                service
                    .uninstall_with_worker_stop(id, version)
                    .await
                    .map(|()| serde_json::json!({ "uninstalled": true })),
            )
        }
        other => LocalResponse::err(format!("packages.{other}: unknown sub-operation")),
    }
}

struct StoreCatalogIndex<'a> {
    packages: &'a PackageService,
}

impl PackageIndexLookup for StoreCatalogIndex<'_> {
    fn package_release(&self, package_id: &str, version: &str, is_bridge: bool) -> bool {
        self.packages
            .has_catalog_release(package_id, version, is_bridge)
    }

    fn canonical_type_version(&self, type_id: &str, versions: &str) -> bool {
        let Ok(requirement) = semver::VersionReq::parse(versions) else {
            return false;
        };
        ark_core::canonical_types::definitions::canonical_type_registrations()
            .ok()
            .is_some_and(|types| {
                types.into_iter().any(|registered| {
                    registered.type_id == type_id
                        && semver::Version::parse(&registered.version)
                            .ok()
                            .is_some_and(|version| requirement.matches(&version))
                })
            })
    }
}

async fn handle_store_op(
    subop: &str,
    params: serde_json::Value,
    packages: &PackageService,
    catalog: Option<&StoreCatalogService>,
) -> LocalResponse {
    if subop == "external_url" {
        let listing_id = params.get("listing_id").and_then(Value::as_str);
        return match (catalog, listing_id) {
            (Some(catalog), Some(listing_id)) => catalog
                .external_url_at(listing_id, chrono::Utc::now())
                .map(|url| LocalResponse::ok(serde_json::json!({ "url": url })))
                .unwrap_or_else(|_| LocalResponse::err("store: unavailable")),
            _ => LocalResponse::err("store: unavailable"),
        };
    }
    let installed = match packages.store_installed_listings() {
        Ok(installed) => installed,
        Err(_) => return LocalResponse::err("store: installed-packages-unavailable"),
    };
    let response = match subop {
        "catalog" => Ok(match catalog {
            Some(catalog) => catalog.catalog(chrono::Utc::now(), installed),
            None => CatalogDto {
                state: "unavailable".into(),
                sequence: None,
                issued_at: None,
                expires_at: None,
                listings: Vec::new(),
                installed,
            },
        }),
        "refresh" => match catalog {
            Some(catalog) => {
                if packages.catalog_summary().is_none() && packages.refresh_catalog().await.is_err()
                {
                    Err("store: package-index-unavailable")
                } else {
                    match catalog.refresh(&StoreCatalogIndex { packages }).await {
                        Ok(_) => Ok(catalog.catalog(
                            chrono::Utc::now(),
                            packages.store_installed_listings().unwrap_or_default(),
                        )),
                        Err(_) => Err("store: refresh-unavailable"),
                    }
                }
            }
            None => Err("store: unavailable"),
        },
        other => return LocalResponse::err(format!("store.{other}: unknown sub-operation")),
    };
    match response
        .and_then(|result| serde_json::to_value(result).map_err(|_| "store: serialization-failed"))
    {
        Ok(value) => LocalResponse::ok(value),
        Err(error) => LocalResponse::err(error),
    }
}

fn package_signatures(params: &serde_json::Value) -> Result<SignatureSet, LocalResponse> {
    params
        .get("signatures")
        .cloned()
        .ok_or_else(|| LocalResponse::err("packages: invalid-request"))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|_| LocalResponse::err("packages: invalid-request"))
        })
}

async fn package_blocking<T, F>(work: F) -> Result<T, PackageError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PackageError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| PackageError::Persistence)?
}

fn package_response<T: serde::Serialize>(
    operation: &str,
    result: Result<T, PackageError>,
) -> LocalResponse {
    match result {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => LocalResponse::ok(value),
            Err(_) => LocalResponse::err(format!("packages.{operation}: serialization-failed")),
        },
        Err(error) => LocalResponse::err(format!(
            "packages.{operation}: {}",
            package_error_code(&error)
        )),
    }
}

fn package_error_code(error: &PackageError) -> &'static str {
    match error {
        PackageError::TrustUnavailable => "trust-unavailable",
        PackageError::Invalid => "invalid-request",
        PackageError::Persistence => "persistence-failed",
        PackageError::Trust(TrustError::Expired) => "catalog-expired",
        PackageError::Trust(TrustError::Replay) => "replay-rejected",
        PackageError::Trust(TrustError::RevokedKey | TrustError::RevokedPackage) => "revoked",
        PackageError::Trust(_) => "trust-rejected",
        PackageError::Store(_) => "store-rejected",
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
            if !converter.supported_formats().contains(&format.as_str()) {
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
            let objects: Vec<ark_core::types::ArkObject> =
                match serde_json::from_value(ark_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "export.run: parse ArkObject array: {e}"
                        ))
                    }
                };

            let links_resp = match ark_host
                .request("list_object_links", serde_json::json!({}))
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("export.run: ark_host links: {e}")),
            };
            if !links_resp.ok {
                return LocalResponse::err(format!(
                    "export.run: ark list_object_links failed: {}",
                    links_resp.error.unwrap_or_default()
                ));
            }
            let links: Vec<ark_core::types::ObjectLink> =
                match serde_json::from_value(links_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "export.run: parse ObjectLink array: {e}"
                        ))
                    }
                };
            let envelopes = objects
                .into_iter()
                .map(|object| export::CanonicalEnvelope {
                    links: links
                        .iter()
                        .filter(|link| link.source_object_id == object.id)
                        .cloned()
                        .collect(),
                    object,
                })
                .collect::<Vec<_>>();
            let result = converter.convert_canonical(&envelopes, &format, &dest_dir);
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
///   - `arrancador.scan` → сканирует Steam/Epic, persists through Game facade.
///   - `arrancador.launch { game_id }` → reads typed Game DTO, then spawns
///     процесс через `launcher::launch`.
///
/// `arrancador.rawg.*` и `arrancador.sqoba.*` будут добавлены subagent'ами B/C
/// в этот же match (один namespace, один диспатчер).
async fn handle_arrancador_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    let game_facade = arrancador::game_facade::GameFacade::new(ark_host);
    match subop {
        "list" | "read" => {
            let result = if subop == "read" {
                let id = match params.get("id").and_then(Value::as_str) {
                    Some(id) => id,
                    None => return LocalResponse::err("arrancador.read: missing id"),
                };
                game_facade.read(id).await
            } else {
                game_facade.list().await
            };
            match result {
                Ok(value) => LocalResponse::ok(value),
                Err(error) => LocalResponse::err(format!("arrancador.{subop}: {error}")),
            }
        }
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

            // Read only canonical Games. Launcher/provider identity is owned by
            // the device-local Arrancador state, never by canonical props.
            let existing = match game_facade.objects().await {
                Ok(objects) => objects,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador: game facade: {error}"))
                }
            };
            let mut cfg = arrancador::config::load();
            let mut added = 0u32;
            let mut updated = 0u32;
            let mut skipped = 0u32;
            let mut errors: Vec<String> = Vec::new();

            for game in &discovered {
                let existing_match = existing.iter().find(|obj| {
                    cfg.local_games
                        .get(&obj.id)
                        .and_then(|s| s.source.as_deref())
                        == Some(game.source.as_str())
                        && cfg
                            .local_games
                            .get(&obj.id)
                            .and_then(|s| s.source_app_id.as_deref())
                            == Some(game.source_app_id.as_str())
                });
                let props = serde_json::json!({ "platforms": [] });
                let now = chrono::Utc::now().to_rfc3339();
                let id = existing_match
                    .map(|obj| obj.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let upsert_obj = arrancador::game_facade::GameFacade::upsert_payload(
                    existing_match,
                    &id,
                    &game.name,
                    &props,
                    &now,
                );
                let is_new = existing_match.is_none();
                match game_facade.upsert(upsert_obj).await {
                    Ok(_) => {
                        cfg.local_games.insert(
                            id,
                            arrancador::config::LocalGameState {
                                source: Some(game.source.clone()),
                                source_app_id: Some(game.source_app_id.clone()),
                                install_dir: Some(game.install_dir.to_string_lossy().to_string()),
                                exe_path: game
                                    .exe_candidate
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().to_string()),
                                save_paths: Vec::new(),
                            },
                        );
                        if is_new {
                            added += 1;
                        } else {
                            updated += 1;
                        }
                    }
                    Err(e) => {
                        skipped += 1;
                        errors.push(format!("{}: ark_host: {e}", game.name));
                    }
                }
            }
            if let Err(e) = arrancador::config::save(&cfg) {
                return LocalResponse::err(format!(
                    "arrancador.scan: local state save failed: {e}"
                ));
            }

            LocalResponse::ok(serde_json::json!({
                "added": added,
                "updated": updated,
                "skipped": skipped,
                "discovered": discovered.len(),
                "errors": errors,
            }))
        }
        "add_manual" => {
            let name = match params.get("name").and_then(|v| v.as_str()).map(str::trim) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return LocalResponse::err("arrancador.add_manual: missing 'name'"),
            };
            let input_path = match params
                .get("exe_path")
                .and_then(|v| v.as_str())
                .map(str::trim)
            {
                Some(s) if !s.is_empty() => std::path::PathBuf::from(s),
                _ => return LocalResponse::err("arrancador.add_manual: missing 'exe_path'"),
            };
            let exe_path = match resolve_arrancador_manual_exec_path(&input_path) {
                Ok(path) => path,
                Err(e) => return LocalResponse::err(format!("arrancador.add_manual: {e}")),
            };
            let save_paths: Vec<std::path::PathBuf> = params
                .get("save_paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::trim))
                        .filter(|s| !s.is_empty())
                        .map(std::path::PathBuf::from)
                        .collect()
                })
                .unwrap_or_default();

            let props = serde_json::json!({
                "playStatus": "notStarted",
            });
            let existing = match game_facade.objects().await {
                Ok(objects) => objects,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador: game facade: {error}"))
                }
            };
            let mut cfg = arrancador::config::load();
            let source_app_id = exe_path.to_string_lossy().to_string();
            let existing_match = existing.iter().find(|obj| {
                cfg.local_games
                    .get(&obj.id)
                    .and_then(|s| s.source.as_deref())
                    == Some("manual")
                    && cfg
                        .local_games
                        .get(&obj.id)
                        .and_then(|s| s.source_app_id.as_deref())
                        == Some(source_app_id.as_str())
            });
            let now = chrono::Utc::now().to_rfc3339();
            let id = existing_match
                .map(|obj| obj.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let object = arrancador::game_facade::GameFacade::upsert_payload(
                existing_match,
                &id,
                &name,
                &props,
                &now,
            );
            match game_facade.upsert(object).await {
                Ok(_) => {
                    cfg.local_games.insert(
                        id.clone(),
                        arrancador::config::LocalGameState {
                            source: Some("manual".into()),
                            source_app_id: Some(source_app_id),
                            install_dir: exe_path.parent().map(|p| p.to_string_lossy().to_string()),
                            exe_path: Some(exe_path.to_string_lossy().to_string()),
                            save_paths: save_paths
                                .into_iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect(),
                        },
                    );
                    if let Err(e) = arrancador::config::save(&cfg) {
                        return LocalResponse::err(format!(
                            "arrancador.add_manual: local state save failed: {e}"
                        ));
                    }
                    LocalResponse::ok(serde_json::json!({ "ok": true, "id": id }))
                }
                Err(e) => LocalResponse::err(format!("arrancador.add_manual: ark_host: {e}")),
            }
        }
        "launch" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.launch: missing 'game_id'"),
            };
            let game = match game_facade.object(&game_id).await {
                Ok(game) => game,
                Err(error) => {
                    return LocalResponse::err(format!("arrancador.launch: game facade: {error}"))
                }
            };
            let local = arrancador::config::load()
                .local_games
                .get(&game.id)
                .cloned()
                .unwrap_or_default();
            let launch_game = arrancador::game_facade::GameFacade::launch_dto(&game);
            match arrancador::launcher::launch(&launch_game, &local) {
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
        "config.get_rawg_key" => {
            let cfg = arrancador::config::load();
            LocalResponse::ok(
                serde_json::json!({ "configured": cfg.rawg_api_key.as_deref().is_some_and(|key| !key.is_empty()) }),
            )
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
                    Err(e) => LocalResponse::err(format!("arrancador.rawg.search: serialize: {e}")),
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
            match arrancador::rawg::apply_to_game(&game_facade, &game_id, rawg_id, &api_key).await {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("arrancador.rawg.apply: {e}")),
            }
        }
        "sqoba.backup" => {
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("arrancador.sqoba.backup: missing 'game_id'"),
            };
            let game = match game_facade.object(&game_id).await {
                Ok(game) => game,
                Err(error) => {
                    return LocalResponse::err(format!(
                        "arrancador.sqoba.backup: game facade: {error}"
                    ))
                }
            };
            let (game_name, manual_paths) =
                arrancador::game_facade::GameFacade::sqoba_metadata(&game);
            match arrancador::sqoba::backup(&game_id, &game_name, manual_paths.as_deref()) {
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
            let game_id = match params.get("game_id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return LocalResponse::err("arrancador.sqoba.restore: missing 'game_id'"),
            };
            let path = match arrancador::sqoba::resolve_backup_path(game_id, &backup_id) {
                Some(p) => p,
                None => {
                    return LocalResponse::err(format!(
                        "arrancador.sqoba.restore: backup '{}' not found for game '{}'",
                        backup_id, game_id
                    ))
                }
            };
            match arrancador::sqoba::restore(&path) {
                Ok(r) => match serde_json::to_value(&r) {
                    Ok(mut v) => {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("ok".into(), serde_json::Value::Bool(r.errors.is_empty()));
                        }
                        LocalResponse::ok(v)
                    }
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

async fn handle_calculator_op(
    subop: &str,
    params: serde_json::Value,
    data_dir: &std::path::Path,
) -> LocalResponse {
    match subop {
        "evaluate" => {
            let query = match params.get("query").and_then(|value| value.as_str()) {
                Some(query) => query.to_string(),
                None => return LocalResponse::err("calculator.evaluate: missing 'query'"),
            };
            let normalized = calculator::normalize_query(&query);
            let rates =
                calculator::exchange_rates_for_query(&normalized.evaluation, data_dir).await;
            let expression = normalized.display_expression;
            let evaluation = normalized.evaluation;
            match tokio::task::spawn_blocking(move || {
                calculator::evaluate_preview_with_rates(&evaluation, rates)
            })
            .await
            {
                Ok(result) => LocalResponse::ok(serde_json::json!({
                    "result": result,
                    "expression": expression,
                })),
                Err(error) => LocalResponse::err(format!("calculator.evaluate: join: {error}")),
            }
        }
        other => LocalResponse::err(format!("calculator.{other}: unknown sub-operation")),
    }
}

/// Dispatch `app_index.<subop>` — App Launcher: search / launch / rescan.
///
/// Sub-operations:
///   - `app_index.search { query, limit? }` → `{ results: [{app, score}] }`
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
            // Lightweight command-list payload: no bulk inline icons in the WS hot path.
            // См. postmortems.md § 2026-06-08 — WS hot path инлайнил сотни иконок.
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            let out: Vec<_> = app_index
                .all(limit)
                .await
                .into_iter()
                .map(|app| app_index_entry_json(&app))
                .collect();
            LocalResponse::ok(serde_json::json!({ "apps": out }))
        }
        "search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(q) => q.to_string(),
                None => return LocalResponse::err("app_index.search: missing 'query'"),
            };
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
            // Frecency: пустой UsageStats в v1. TODO: join из ARK usage_event_obj.
            let usage = UsageStats::empty();
            let results: Vec<_> = app_index
                .search(&query, limit, &usage)
                .await
                .into_iter()
                .map(|scored| {
                    serde_json::json!({
                        "app": app_index_entry_json(&scored.app),
                        "score": scored.score,
                    })
                })
                .collect();
            match serde_json::to_value(serde_json::json!({ "results": results })) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("app_index.search: serialize: {e}")),
            }
        }
        "icon_path" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("app_index.icon_path: missing 'id'"),
            };
            let app = match app_index.find(&id).await {
                Some(a) => a,
                None => return LocalResponse::err(format!("app_index.icon_path: not found: {id}")),
            };
            LocalResponse::ok(serde_json::json!({ "path": app.icon_path }))
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

fn resolve_arrancador_manual_exec_path(
    input_path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    if !input_path.is_file() {
        return Err(format!("path is not a file: {}", input_path.display()));
    }

    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    #[cfg(target_os = "windows")]
    if ext.as_deref() == Some("lnk") {
        let target =
            crate::app_index::platform::windows::start_menu::resolve_lnk_target_path(input_path)
                .map_err(|e| format!("failed to read shortcut: {e}"))?
                .ok_or_else(|| format!("shortcut has no target: {}", input_path.display()))?;
        if !target.is_file() {
            return Err(format!(
                "shortcut target is not a file: {}",
                target.display()
            ));
        }
        return Ok(target);
    }

    match std::fs::canonicalize(input_path) {
        Ok(path) => {
            let value = path.to_string_lossy().to_string();
            if let Some(stripped) = value.strip_prefix(r"\\?\") {
                Ok(std::path::PathBuf::from(stripped))
            } else {
                Ok(path)
            }
        }
        Err(_) => Ok(input_path.to_path_buf()),
    }
}

fn app_icon_ref(app: &crate::app_index::App) -> Option<String> {
    app.icon_path
        .as_ref()
        .map(|_| format!("kosmos-icon://app/{}", app.id))
}

fn app_index_entry_json(app: &crate::app_index::App) -> serde_json::Value {
    serde_json::json!({
        "id": &app.id,
        "name": &app.name,
        "exec_path": &app.exec_path,
        // См. postmortems.md § 2026-06-09: app-index hot paths return refs;
        // renderer loads only visible icons through the Electron protocol.
        "icon_path": null,
        "icon_ref": app_icon_ref(app),
        "kind": &app.kind,
        "source": &app.source,
        "mtime": app.mtime,
    })
}

/// Dispatch `file_index.<subop>` — host-local file search and settings.
async fn handle_file_index_op(
    subop: &str,
    params: serde_json::Value,
    file_index: &Arc<FileIndex>,
) -> LocalResponse {
    match subop {
        "search" => {
            let query = match params.get("query").and_then(|v| v.as_str()) {
                Some(query) => query,
                None => return LocalResponse::err("file_index.search: missing 'query'"),
            };
            let limit = params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(8)
                .min(50) as usize;
            let index = file_index.clone();
            let query = query.to_string();
            match tokio::task::spawn_blocking(move || index.search(&query, limit)).await {
                Ok(Ok(results)) => {
                    match serde_json::to_value(serde_json::json!({ "results": results })) {
                        Ok(value) => LocalResponse::ok(value),
                        Err(e) => LocalResponse::err(format!("file_index.search: serialize: {e}")),
                    }
                }
                Ok(Err(e)) => LocalResponse::err(format!("file_index.search: {e}")),
                Err(e) => LocalResponse::err(format!("file_index.search: join: {e}")),
            }
        }
        "open" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path,
                None => return LocalResponse::err("file_index.open: missing 'path'"),
            };
            match file_index.open(path) {
                Ok(()) => LocalResponse::ok(serde_json::json!({ "ok": true })),
                Err(e) => LocalResponse::err(format!("file_index.open: {e}")),
            }
        }
        "rescan" => match file_index.request_rescan() {
            Ok(stats) => match serde_json::to_value(stats) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.rescan: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.rescan: {e}")),
        },
        "clear_cache" => match file_index.clear_cache() {
            Ok(stats) => match serde_json::to_value(stats) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.clear_cache: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.clear_cache: {e}")),
        },
        "diagnostics" => match file_index.diagnostics() {
            Ok(diag) => match serde_json::to_value(diag) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.diagnostics: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.diagnostics: {e}")),
        },
        "estimate_root" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path.to_string(),
                None => return LocalResponse::err("file_index.estimate_root: missing 'path'"),
            };
            let index = file_index.clone();
            match tokio::task::spawn_blocking(move || index.estimate_root(&path)).await {
                Ok(Ok(estimate)) => match serde_json::to_value(estimate) {
                    Ok(value) => LocalResponse::ok(value),
                    Err(e) => {
                        LocalResponse::err(format!("file_index.estimate_root: serialize: {e}"))
                    }
                },
                Ok(Err(e)) => LocalResponse::err(format!("file_index.estimate_root: {e}")),
                Err(e) => LocalResponse::err(format!("file_index.estimate_root: join: {e}")),
            }
        }
        "settings_get" => match file_index.settings() {
            Ok(settings) => match serde_json::to_value(settings) {
                Ok(value) => LocalResponse::ok(value),
                Err(e) => LocalResponse::err(format!("file_index.settings_get: serialize: {e}")),
            },
            Err(e) => LocalResponse::err(format!("file_index.settings_get: {e}")),
        },
        "settings_set" => {
            let patch: FileIndexSettingsPatch = match serde_json::from_value(params) {
                Ok(patch) => patch,
                Err(e) => return LocalResponse::err(format!("file_index.settings_set: {e}")),
            };
            match file_index.set_settings(patch).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.settings_set: {e}")),
            }
        }
        "scope_add" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path,
                None => return LocalResponse::err("file_index.scope_add: missing 'path'"),
            };
            match file_index.add_root(path).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.scope_add: {e}")),
            }
        }
        "scope_remove" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(path) => path,
                None => return LocalResponse::err("file_index.scope_remove: missing 'path'"),
            };
            match file_index.remove_root(path).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.scope_remove: {e}")),
            }
        }
        "ignore_add" => {
            let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
                Some(pattern) => pattern,
                None => return LocalResponse::err("file_index.ignore_add: missing 'pattern'"),
            };
            match file_index.add_ignore_pattern(pattern).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.ignore_add: {e}")),
            }
        }
        "ignore_remove" => {
            let pattern = match params.get("pattern").and_then(|v| v.as_str()) {
                Some(pattern) => pattern,
                None => return LocalResponse::err("file_index.ignore_remove: missing 'pattern'"),
            };
            match file_index.remove_ignore_pattern(pattern).await {
                Ok(stats) => LocalResponse::ok(serde_json::json!({ "stats": stats })),
                Err(e) => LocalResponse::err(format!("file_index.ignore_remove: {e}")),
            }
        }
        other => LocalResponse::err(format!("file_index.{other}: unknown sub-operation")),
    }
}

async fn send_message<S>(
    sink: &mut S,
    message: Message,
    shutdown: &WsShutdownHandle,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    tokio::select! {
        _ = shutdown.cancelled() => Ok(()),
        result = tokio::time::timeout(WS_SEND_DEADLINE, sink.send(message)) => {
            match result {
                Ok(result) => result.map_err(WsServerError::from),
                Err(_) => Err(WsServerError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "WebSocket send timed out",
                    )),
                )),
            }
        }
    }
}

async fn send_hello_error<S>(
    sink: &mut S,
    code: &str,
    message: &str,
    shutdown: &WsShutdownHandle,
) -> Result<(), WsServerError>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let payload = serde_json::to_string(&HelloErrorResponse {
        kind: "hello_error",
        code,
        message: message.to_string(),
    })?;
    send_message(sink, Message::Text(payload), shutdown).await?;
    send_message(sink, Message::Close(None), shutdown).await?;
    Ok(())
}

// ----- Unit tests: hello validation без network -----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_message_limit_accepts_five_minute_dictation_wav() {
        // Regression: 2026-08-19. Base64 WAV used to exceed the 1 MiB WS limit
        // and tungstenite closed the whole Engine connection before dispatch.
        const PCM_BYTES_PER_SECOND: usize = 16_000 * 2;
        const FIVE_MINUTE_WAV_BASE64_BYTES: usize = ((44 + PCM_BYTES_PER_SECOND * 300) + 2) / 3 * 4;
        assert!(MAX_WS_MESSAGE_BYTES >= FIVE_MINUTE_WAV_BASE64_BYTES + 1024);
    }
    use crate::app_index::{App, AppKind};
    use tokio::io::AsyncReadExt;

    fn baseline_hello() -> HelloMessage {
        HelloMessage {
            kind: Some("hello".into()),
            protocol_version: None,
            api_version: Some(API_VERSION.into()),
            token: Some("test-token".into()),
            pid: Some(std::process::id()),
            client_id: Some("eden".into()),
            client_class: Some("@kosmos/ark".into()),
            client_version: Some("0.1.0".into()),
        }
    }

    fn accepted_compat(outcome: HelloOutcome) -> Compatibility {
        let HelloOutcome::Accept { compatibility, .. } = outcome else {
            assert!(
                matches!(outcome, HelloOutcome::Accept { .. }),
                "expected accept"
            );
            unreachable!();
        };
        compatibility
    }

    fn rejected_code(outcome: HelloOutcome) -> &'static str {
        let HelloOutcome::Reject { code, .. } = outcome else {
            assert!(
                matches!(outcome, HelloOutcome::Reject { .. }),
                "expected reject"
            );
            unreachable!();
        };
        code
    }

    #[test]
    fn legacy_protocol_version_requires_upgrade() {
        let mut hello = baseline_hello();
        hello.api_version = None;
        hello.protocol_version = Some("1.0.0".into());
        let outcome = validate_hello(&hello, "test-token");
        assert_eq!(rejected_code(outcome), handshake_errors::UPGRADE_REQUIRED);
    }

    #[test]
    fn valid_api_v1_hello_accepted() {
        let hello = baseline_hello();
        let outcome = validate_hello(&hello, "test-token");
        assert!(matches!(
            outcome,
            HelloOutcome::Accept {
                compatibility: Compatibility::Exact,
                transport: TransportKind::ApiV1
            }
        ));
    }

    #[test]
    fn api_v1_missing_hello_kind_is_rejected() {
        let mut hello = baseline_hello();
        hello.kind = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MALFORMED_HELLO
        );
    }

    #[test]
    fn legacy_missing_hello_kind_requires_upgrade() {
        let mut hello = baseline_hello();
        hello.api_version = None;
        hello.protocol_version = Some("1.0.0".into());
        hello.kind = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::UPGRADE_REQUIRED
        );
    }

    #[test]
    fn hello_with_both_versions_is_rejected() {
        let mut hello = baseline_hello();
        hello.protocol_version = Some("1.0.0".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::UPGRADE_REQUIRED
        );
    }

    #[test]
    fn missing_protocol_version_rejected() {
        let mut hello = baseline_hello();
        hello.api_version = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MISSING_PROTOCOL_VERSION
        );
    }

    #[test]
    fn malformed_protocol_version_rejected() {
        let mut hello = baseline_hello();
        hello.api_version = Some("not-a-version".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MALFORMED_PROTOCOL_VERSION
        );
    }

    #[test]
    fn major_mismatch_rejected_as_incompatible() {
        let mut hello = baseline_hello();
        hello.api_version = Some("2.0.0".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::INCOMPATIBLE_PROTOCOL_VERSION
        );
    }

    #[test]
    fn minor_mismatch_accepted() {
        let mut hello = baseline_hello();
        hello.api_version = Some("1.99.0".into());
        assert_eq!(
            accepted_compat(validate_hello(&hello, "test-token")),
            Compatibility::MinorMismatch
        );
    }

    #[test]
    fn missing_token_rejected() {
        let mut hello = baseline_hello();
        hello.token = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MISSING_TOKEN
        );
    }

    #[test]
    fn invalid_token_rejected() {
        let mut hello = baseline_hello();
        hello.token = Some("wrong-token".into());
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::INVALID_TOKEN
        );
    }

    #[test]
    fn missing_pid_rejected() {
        let mut hello = baseline_hello();
        hello.pid = None;
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::MISSING_PID
        );
    }

    #[test]
    fn nonexistent_pid_rejected() {
        let mut hello = baseline_hello();
        hello.pid = Some(0x7FFFFFFF); // impossibly high
        assert_eq!(
            rejected_code(validate_hello(&hello, "test-token")),
            handshake_errors::INVALID_PID
        );
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

    #[test]
    fn app_index_entry_uses_icon_ref_without_inline_data_url() {
        // Regression: 2026-06-09. app_index.search must not read/base64 top-N icons.
        let app = App {
            id: "calc".into(),
            name: "Calculator".into(),
            exec_path: "C:\\Windows\\System32\\calc.exe".into(),
            icon_path: Some("C:\\Kosmos\\icons\\calc.png".into()),
            icon_source: None,
            kind: AppKind::Win32,
            source: "test".into(),
            mtime: 1,
        };

        let entry = app_index_entry_json(&app);

        assert_eq!(entry["icon_path"], serde_json::Value::Null);
        assert_eq!(entry["icon_ref"], "kosmos-icon://app/calc");
    }

    #[tokio::test]
    async fn calculator_op_returns_result_or_quiet_null() {
        let data_dir = tempfile::tempdir().unwrap();
        let result = handle_calculator_op(
            "evaluate",
            serde_json::json!({ "query": "1200 * 1.2" }),
            data_dir.path(),
        )
        .await;
        assert!(result.ok);
        assert_eq!(result.data["result"], "1440");
        assert_eq!(result.data["expression"], "1200 * 1.2");

        let search_text = handle_calculator_op(
            "evaluate",
            serde_json::json!({ "query": "settings" }),
            data_dir.path(),
        )
        .await;
        assert!(search_text.ok);
        assert!(search_text.data["result"].is_null());
    }

    #[tokio::test]
    async fn file_index_diagnostics_op_returns_enriched_payload() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("diag-note.md"), "v1").unwrap();
        let index =
            Arc::new(FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap());
        index.rescan().await.unwrap();

        let response = handle_file_index_op("diagnostics", serde_json::Value::Null, &index).await;

        assert!(response.ok);
        assert_eq!(response.data["roots_count"], 1);
        assert_eq!(response.data["files_count"], 1);
        assert!(response.data.get("db_size_bytes").is_some());
    }

    #[tokio::test]
    async fn file_index_estimate_root_op_returns_estimate_payload() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("notes.md"), "v1").unwrap();
        std::fs::write(root.path().join("photo.png"), "v1").unwrap();
        let index =
            Arc::new(FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap());

        let response = handle_file_index_op(
            "estimate_root",
            serde_json::json!({ "path": root.path().to_string_lossy() }),
            &index,
        )
        .await;

        assert!(response.ok);
        assert_eq!(response.data["indexable_text_files_count"], 1);
        assert_eq!(response.data["metadata_only_media_files_count"], 1);
    }

    #[tokio::test]
    async fn package_api_returns_bounded_metadata_without_trust_material_or_paths() {
        let data = tempfile::tempdir().unwrap();
        let service = Arc::new(PackageService::open(data.path()).unwrap());

        let status = handle_package_op("trust_status", serde_json::Value::Null, &service).await;
        assert!(status.ok);
        let status_json = status.data.to_string();
        for forbidden in [
            "public_key",
            "signature",
            "archive_path",
            "entrypoint",
            "permissions",
            "sha256",
        ] {
            assert!(!status_json.contains(forbidden), "{status_json}");
        }

        let list = handle_package_op("list", serde_json::Value::Null, &service).await;
        assert!(list.ok);
        assert_eq!(list.data["total"], 0);
        assert_eq!(list.data["truncated"], false);

        let private_path = r"C:\Users\alice\private\package.kspkg";
        let install = handle_package_op(
            "install",
            serde_json::json!({
                "id": "com.kosmos.demo",
                "version": "1.0.0",
                "archive_path": private_path,
            }),
            &service,
        )
        .await;
        assert!(!install.ok);
        assert!(!install.error.unwrap_or_default().contains(private_path));
    }

    async fn production_ws_fixture() -> (tempfile::TempDir, WsServer) {
        let dir = tempfile::tempdir().unwrap();
        let binary = crate::ark_host::resolve_ark_core_rpc_path()
            .expect("real ark-core-rpc fixture must be built");
        let ark = Arc::new(
            crate::ark_host::ArkHost::spawn(&binary, &dir.path().join("ark.db").to_string_lossy())
                .await
                .unwrap(),
        );
        let ws = WsServer::bind(
            ark,
            "test-token".repeat(8),
            dir.path().to_path_buf(),
            Arc::new(
                crate::app_index::AppIndex::new(dir.path(), dir.path().join("icons")).unwrap(),
            ),
            Arc::new(crate::file_index::FileIndex::new_disabled(dir.path()).unwrap()),
            Arc::new(crate::usage_tracker::UsageTrackerDiagnosticsState::default()),
            Arc::new(crate::protocol_usage::ProtocolUsageStore::open(dir.path()).unwrap()),
            Arc::new(crate::package_service::PackageService::open(dir.path()).unwrap()),
            "00000000-0000-4000-8000-000000000001".into(),
        )
        .await
        .unwrap();
        (dir, ws)
    }

    #[tokio::test]
    async fn shutdown_reports_deadline_breach_after_forced_reap_and_is_idempotent() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        let task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        handle
            .lifecycle
            .tasks
            .lock()
            .unwrap()
            .insert(1, WsConnectionSlot::Installed(task));

        let started = Instant::now();
        assert!(handle.drain(Duration::from_millis(5)).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(handle.task_count(), 0);
        assert!(handle.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn ws_reserved_shutdown_releases_socket_owner_and_permit_before_drain_returns() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        let allocator = crate::engine_dispatch::OwnerAllocator::default();
        let owner = allocator.allocate().unwrap();
        let permit = handle
            .lifecycle
            .capacity
            .clone()
            .try_acquire_owned()
            .unwrap();
        let (start, _started) = tokio::sync::oneshot::channel();
        handle.lifecycle.tasks.lock().unwrap().insert(
            1,
            WsConnectionSlot::Reserved {
                resources: Arc::new(Mutex::new(Some(WsConnectionResources {
                    stream: None,
                    permit: Some(permit),
                    owner_lease: Some(owner),
                }))),
                start,
            },
        );

        handle.shutdown().await.unwrap();

        assert_eq!(handle.task_count(), 0);
        assert_eq!(handle.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
        assert_eq!(allocator.live_count(), 0);
    }

    #[tokio::test]
    async fn ws_preinstall_failure_drops_reserved_resources_without_spawn() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        let allocator = crate::engine_dispatch::OwnerAllocator::default();
        let owner = allocator.allocate().unwrap();
        let permit = handle
            .lifecycle
            .capacity
            .clone()
            .try_acquire_owned()
            .unwrap();
        let (start, _started) = tokio::sync::oneshot::channel();
        handle.lifecycle.tasks.lock().unwrap().insert(
            1,
            WsConnectionSlot::Reserved {
                resources: Arc::new(Mutex::new(Some(WsConnectionResources {
                    stream: None,
                    permit: Some(permit),
                    owner_lease: Some(owner),
                }))),
                start,
            },
        );
        let slot = handle.lifecycle.tasks.lock().unwrap().remove(&1).unwrap();
        drop(slot);

        assert_eq!(handle.task_count(), 0);
        assert_eq!(handle.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
        assert_eq!(allocator.live_count(), 0);
    }

    #[tokio::test]
    async fn ws_shutdown_waits_for_reserved_admission_before_closing_and_draining() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        let admission = handle.lifecycle.admission.lock().unwrap();
        let lifecycle = handle.lifecycle.clone();
        let shutdown = std::thread::spawn(move || {
            let _admission = lifecycle.admission.lock().unwrap();
            lifecycle.closed.store(true, Ordering::Release);
        });
        std::thread::yield_now();
        assert!(!handle.lifecycle.closed.load(Ordering::Acquire));
        drop(admission);
        shutdown.join().unwrap();
        handle.shutdown().await.unwrap();
        assert!(handle.lifecycle.closed.load(Ordering::Acquire));
        assert_eq!(handle.task_count(), 0);
        assert_eq!(handle.request_task_count(), 0);
    }

    #[tokio::test]
    async fn ws_immediate_connection_tasks_are_joined_and_reaped() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        let task = tokio::spawn(async {});
        handle
            .lifecycle
            .tasks
            .lock()
            .unwrap()
            .insert(1, WsConnectionSlot::Installed(task));
        tokio::task::yield_now().await;
        handle.reap().await;
        assert_eq!(handle.task_count(), 0);
    }

    #[tokio::test]
    async fn ws_connection_registry_stays_bounded_under_10k_immediate_accept_close_cycles() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        for id in 1..=10_000 {
            let task = tokio::spawn(async {});
            handle
                .lifecycle
                .tasks
                .lock()
                .unwrap()
                .insert(id, WsConnectionSlot::Installed(task));
            tokio::task::yield_now().await;
            handle.reap().await;
            assert!(handle.task_count() <= 1);
        }
        assert_eq!(handle.task_count(), 0);
    }

    #[tokio::test]
    async fn shutdown_admission_race_cannot_spawn_after_request_drain() {
        let handle = WsShutdownHandle {
            lifecycle: Arc::new(WsLifecycle::default()),
        };
        handle.shutdown().await.unwrap();
        let spawned = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let spawned_clone = spawned.clone();
        let permit = Arc::new(Mutex::new(Some(
            handle
                .lifecycle
                .request_capacity
                .clone()
                .try_acquire_owned()
                .unwrap(),
        )));
        let (cancel_sender, _cancel_receiver) = tokio::sync::oneshot::channel();
        let installed = handle.install_request(
            1,
            permit,
            Arc::new(Mutex::new(Some(cancel_sender))),
            move |start_receiver| {
                spawned_clone.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let _ = start_receiver.await;
                })
            },
        );
        assert!(!installed);
        assert_eq!(spawned.load(Ordering::SeqCst), 0);
        assert_eq!(handle.request_task_count(), 0);
        assert_eq!(
            handle.lifecycle.request_capacity.available_permits(),
            MAX_ACTIVE_WS_REQUESTS
        );
    }

    #[tokio::test]
    async fn production_ws_shutdown_reaps_authenticated_and_stalled_lifecycles() {
        let (_dir, server) = production_ws_fixture().await;
        let shutdown = server.shutdown_handle();
        shutdown.set_response_deadline(Duration::from_millis(50));
        let port = server.port();
        let dispatcher = server.dispatcher();
        let bus = server.command_bus_handle();
        let task = tokio::spawn(server.run());

        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({
                    "kind": "hello",
                    "apiVersion": API_VERSION,
                    "token": "test-token".repeat(8),
                    "pid": std::process::id(),
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert!(socket
            .next()
            .await
            .unwrap()
            .unwrap()
            .to_text()
            .unwrap()
            .contains("hello_ok"));
        socket
            .send(Message::Text(
                r#"{"operation":"commands.register","commands":[{"id":"lifecycle.command","title":"Lifecycle","category":"test"}]}"#.into(),
            ))
            .await
            .unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while bus.registration_count_sync() != 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        let mut raw = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let (mut no_hello, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();
        let _ = (&mut raw, &mut no_hello);
        tokio::time::timeout(Duration::from_secs(1), async {
            while shutdown.task_count() < 3 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();

        shutdown.begin_shutdown().await;
        task.await.unwrap().unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .is_ok());
        let first_shutdown = shutdown.shutdown().await;
        assert!(
            first_shutdown.is_err(),
            "stalled lifecycle must report deadline breach"
        );
        shutdown.shutdown().await.unwrap();
        assert_eq!(shutdown.task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(bus.registration_count_sync(), 0);
    }

    #[tokio::test]
    async fn production_ws_capacity_rejects_the_next_raw_socket_and_restores_capacity() {
        let (_dir, server) = production_ws_fixture().await;
        let port = server.port();
        let dispatcher = server.dispatcher();
        let shutdown = server.shutdown_handle();
        let task = tokio::spawn(server.run());
        let mut sockets = Vec::with_capacity(MAX_ACTIVE_WS_CONNECTIONS + 1);
        for _ in 0..=MAX_ACTIVE_WS_CONNECTIONS {
            sockets.push(
                tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .unwrap(),
            );
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while shutdown.task_count() != MAX_ACTIVE_WS_CONNECTIONS {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let mut rejected = sockets.pop().unwrap();
        let mut byte = [0u8; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), rejected.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        shutdown.shutdown().await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(shutdown.task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(shutdown.available_capacity(), MAX_ACTIVE_WS_CONNECTIONS);
    }

    async fn authenticated_socket(
        port: u16,
        token: &str,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({
                    "kind": "hello",
                    "apiVersion": API_VERSION,
                    "token": token,
                    "pid": std::process::id(),
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert!(socket
            .next()
            .await
            .unwrap()
            .unwrap()
            .to_text()
            .unwrap()
            .contains("hello_ok"));
        socket
    }

    #[tokio::test]
    async fn production_ws_reaps_each_sequential_request_on_one_connection() {
        let (_dir, server) = production_ws_fixture().await;
        let shutdown = server.shutdown_handle();
        let dispatcher = server.dispatcher();
        let bus = server.command_bus_handle();
        let port = server.port();
        let task = tokio::spawn(server.run());
        let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;

        for id in 0..10_000_u32 {
            socket
                .send(Message::Text(
                    serde_json::json!({
                        "id": id.to_string(),
                        "operation": "commands.unregister",
                        "ids": [],
                    })
                    .to_string(),
                ))
                .await
                .unwrap();
            loop {
                let response = socket.next().await.unwrap().unwrap();
                let value: serde_json::Value =
                    serde_json::from_str(response.to_text().unwrap()).unwrap();
                if value.get("id") == Some(&serde_json::Value::String(id.to_string())) {
                    assert_eq!(value["ok"], true);
                    break;
                }
            }
            assert_eq!(shutdown.request_task_count(), 0);
        }

        assert_eq!(shutdown.request_task_count(), 0);
        assert_eq!(
            shutdown.lifecycle.request_capacity.available_permits(),
            MAX_ACTIVE_WS_REQUESTS
        );
        drop(socket);
        shutdown.shutdown().await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(shutdown.task_count(), 0);
        assert_eq!(shutdown.request_task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(bus.registration_count_sync(), 0);
    }

    #[tokio::test]
    async fn production_ws_disconnect_cancels_stalled_request_without_replay() {
        let (_dir, server) = production_ws_fixture().await;
        let shutdown = server.shutdown_handle();
        let dispatcher = server.dispatcher();
        let bus = server.command_bus_handle();
        let entered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let entered_observer = entered.clone();
        dispatcher.set_test_observer(Some(Arc::new(move |phase, _, operation| {
            if matches!(phase, crate::engine_dispatch::DispatchPhase::Started)
                && operation == "test.stall"
            {
                entered_observer.fetch_add(1, Ordering::SeqCst);
            }
        })));
        let port = server.port();
        let task = tokio::spawn(server.run());
        let mut socket = authenticated_socket(port, &"test-token".repeat(8)).await;
        socket
            .send(Message::Text(
                serde_json::json!({
                    "id": "stalled",
                    "operation": "test.stall",
                })
                .to_string(),
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while shutdown.request_task_count() != 1 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        socket
            .send(Message::Text(
                serde_json::json!({
                    "id": "second",
                    "operation": "test.stall",
                })
                .to_string(),
            ))
            .await
            .unwrap();
        let busy = tokio::time::timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let busy: serde_json::Value = serde_json::from_str(busy.to_text().unwrap()).unwrap();
        assert_eq!(busy["ok"], false);
        assert_eq!(busy["error"], "WS request busy");
        tokio::time::timeout(Duration::from_secs(1), async {
            while entered.load(Ordering::SeqCst) != 1 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        drop(socket);
        tokio::time::timeout(Duration::from_secs(1), async {
            while shutdown.request_task_count() != 0 || dispatcher.live_owner_count() != 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        shutdown.shutdown().await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(shutdown.request_task_count(), 0);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(bus.registration_count_sync(), 0);
    }
}
