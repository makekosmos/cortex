use super::types::WsLifecycle;
use super::*;

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
