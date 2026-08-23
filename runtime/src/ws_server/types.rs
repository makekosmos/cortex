use super::*;

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
pub(super) const MAX_WS_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_ACTIVE_WS_CONNECTIONS: usize = 128;
pub(super) const MAX_ACTIVE_WS_REQUESTS: usize = 128;
pub(super) const WS_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);
pub(super) const WS_SEND_DEADLINE: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum WsServerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(super) struct WsLifecycle {
    pub(super) admission: Mutex<()>,
    pub(super) closed: AtomicBool,
    pub(super) shutdown: tokio::sync::Notify,
    pub(super) capacity: Arc<tokio::sync::Semaphore>,
    pub(super) tasks: Mutex<HashMap<u64, WsConnectionSlot>>,
    pub(super) next_task: AtomicU64,
    pub(super) request_closed: AtomicBool,
    pub(super) request_capacity: Arc<tokio::sync::Semaphore>,
    pub(super) request_tasks: Mutex<HashMap<u64, WsRequestSlot>>,
    pub(super) next_request: AtomicU64,
    pub(super) response_deadline: Mutex<Duration>,
}

pub(super) struct WsConnectionResources {
    pub(super) stream: Option<tokio::net::TcpStream>,
    pub(super) permit: Option<tokio::sync::OwnedSemaphorePermit>,
    pub(super) owner_lease: Option<crate::engine_dispatch::OwnerLease>,
}

pub(super) enum WsConnectionSlot {
    Reserved {
        resources: Arc<Mutex<Option<WsConnectionResources>>>,
        start: tokio::sync::oneshot::Sender<()>,
    },
    Installed(tokio::task::JoinHandle<()>),
}

pub(super) enum WsRequestSlot {
    Reserved {
        permit: Arc<Mutex<Option<tokio::sync::OwnedSemaphorePermit>>>,
        cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    },
    Installed {
        task: tokio::task::JoinHandle<()>,
        cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    },
}
