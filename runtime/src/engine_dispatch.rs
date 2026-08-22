use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use serde_json::Value;
use thiserror::Error;

pub type DispatchFuture = Pin<Box<dyn Future<Output = DispatchResult> + Send>>;
pub type DispatchHandler = Arc<dyn Fn(DispatchRequest) -> DispatchFuture + Send + Sync>;
pub type CleanupHandler = Arc<dyn Fn(u64) + Send + Sync>;

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchPhase {
    Started,
    Finished,
}

#[cfg(test)]
pub type DispatchObserver = Arc<dyn Fn(DispatchPhase, u64, &str) + Send + Sync>;

#[derive(Clone)]
pub struct OwnerAllocator(Arc<Mutex<OwnerAllocatorState>>);

pub struct OwnerLease {
    allocator: OwnerAllocator,
    id: u64,
    released: AtomicBool,
    cleanup_started: AtomicBool,
}

impl OwnerLease {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn release(&self) {
        if !self.released.swap(true, Ordering::AcqRel) {
            self.allocator.release(self.id);
        }
    }
}

impl Drop for OwnerLease {
    fn drop(&mut self) {
        self.release();
    }
}

#[derive(Default)]
struct OwnerAllocatorState {
    next: u64,
    live: HashSet<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("owner allocator exhausted")]
pub struct OwnerAllocationError;

impl Default for OwnerAllocator {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(OwnerAllocatorState {
            next: 1,
            live: HashSet::new(),
        })))
    }
}

impl OwnerAllocator {
    pub fn allocate(&self) -> Result<OwnerLease, OwnerAllocationError> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.live.len() == (u64::MAX as usize) {
            return Err(OwnerAllocationError);
        }
        let start = if state.next == 0 { 1 } else { state.next };
        let mut candidate = start;
        loop {
            if candidate != 0 && state.live.insert(candidate) {
                state.next = candidate.wrapping_add(1);
                if state.next == 0 {
                    state.next = 1;
                }
                return Ok(OwnerLease {
                    allocator: self.clone(),
                    id: candidate,
                    released: AtomicBool::new(false),
                    cleanup_started: AtomicBool::new(false),
                });
            }
            candidate = candidate.wrapping_add(1);
            if candidate == 0 {
                candidate = 1;
            }
            if candidate == start {
                return Err(OwnerAllocationError);
            }
        }
    }

    pub fn release(&self, owner: u64) {
        if owner == 0 {
            return;
        }
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .live
            .remove(&owner);
    }

    pub fn live_count(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .live
            .len()
    }

    #[cfg(test)]
    fn with_next(next: u64) -> Self {
        Self(Arc::new(Mutex::new(OwnerAllocatorState {
            next,
            live: HashSet::new(),
        })))
    }

    #[cfg(test)]
    fn with_next_and_live(next: u64, live: &[u64]) -> Self {
        Self(Arc::new(Mutex::new(OwnerAllocatorState {
            next,
            live: live.iter().copied().collect(),
        })))
    }

    #[cfg(test)]
    fn set_next_for_test(&self, next: u64) {
        self.0.lock().unwrap().next = next;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Named(String),
}

impl Operation {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Named(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DispatchRequest {
    pub request_id: Option<String>,
    pub operation: Operation,
    pub params: Value,
    pub client: DispatchClient,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispatchClient {
    pub pid: Option<u32>,
    pub class: Option<String>,
    pub version: Option<String>,
    pub correlation_id: Option<String>,
    pub connection_id: Option<u64>,
    pub desktop_authorized: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchValidationError {
    #[error("request must be a JSON object")]
    NotObject,
    #[error("missing 'operation' field")]
    MissingOperation,
    #[error("operation must be a non-empty string")]
    InvalidOperation,
    #[error("request id must be a string")]
    InvalidRequestId,
}

impl DispatchRequest {
    pub fn from_wire(value: Value) -> Result<Self, DispatchValidationError> {
        let mut object = value
            .as_object()
            .cloned()
            .ok_or(DispatchValidationError::NotObject)?;
        let request_id = match object.remove("_req_id") {
            None => match object.remove("id") {
                None => None,
                Some(Value::String(value)) => Some(value),
                Some(_) => return Err(DispatchValidationError::InvalidRequestId),
            },
            Some(Value::String(value)) => Some(value),
            Some(_) => return Err(DispatchValidationError::InvalidRequestId),
        };
        let operation = match object.remove("operation") {
            Some(Value::String(value)) if !value.trim().is_empty() => Operation::Named(value),
            Some(_) => return Err(DispatchValidationError::InvalidOperation),
            None => return Err(DispatchValidationError::MissingOperation),
        };
        let params = Value::Object(object);
        Ok(Self {
            request_id,
            operation,
            params,
            client: DispatchClient::default(),
        })
    }

    pub fn with_client(mut self, client: DispatchClient) -> Self {
        self.client = client;
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

pub type DispatchResult = Result<Value, DispatchError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DispatchError {
    #[error("dispatcher is unavailable")]
    Unavailable,
    #[error("dispatch timed out")]
    Timeout,
    #[error("dispatch cancelled")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}

#[derive(Clone)]
pub struct EngineDispatcher {
    handler: DispatchHandler,
    cleanup: CleanupHandler,
    owners: OwnerAllocator,
    #[cfg(test)]
    observer: Arc<Mutex<Option<DispatchObserver>>>,
}

impl EngineDispatcher {
    pub fn new(handler: DispatchHandler) -> Self {
        Self {
            handler,
            cleanup: Arc::new(|_| {}),
            owners: OwnerAllocator::default(),
            #[cfg(test)]
            observer: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_cleanup(handler: DispatchHandler, cleanup: CleanupHandler) -> Self {
        Self {
            handler,
            cleanup,
            owners: OwnerAllocator::default(),
            #[cfg(test)]
            observer: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn dispatch(&self, request: DispatchRequest) -> DispatchResult {
        #[cfg(test)]
        let observer = self
            .observer
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        #[cfg(test)]
        if let Some(observer) = &observer {
            observer(
                DispatchPhase::Started,
                request.client.connection_id.unwrap_or_default(),
                request.operation.as_str(),
            );
        }
        #[cfg(test)]
        let owner_id = request.client.connection_id.unwrap_or_default();
        #[cfg(test)]
        let operation = request.operation.as_str().to_string();
        let result = (self.handler)(request).await;
        #[cfg(test)]
        if let Some(observer) = &observer {
            observer(DispatchPhase::Finished, owner_id, &operation);
        }
        result
    }

    #[cfg(test)]
    pub fn set_test_observer(&self, observer: Option<DispatchObserver>) {
        *self.observer.lock().unwrap_or_else(|p| p.into_inner()) = observer;
    }

    pub fn cleanup_connection_sync(&self, lease: &OwnerLease) {
        if !lease.cleanup_started.swap(true, Ordering::AcqRel) {
            (self.cleanup)(lease.id());
        }
        lease.release();
    }
    pub fn allocate_owner(&self) -> Result<OwnerLease, OwnerAllocationError> {
        self.owners.allocate()
    }

    pub fn live_owner_count(&self) -> usize {
        self.owners.live_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;
    use std::sync::Mutex;

    #[test]
    fn owner_allocator_never_returns_zero_and_skips_wrap() {
        let allocator = OwnerAllocator::default();
        assert_ne!(allocator.allocate().unwrap().id(), 0);
        let wrapping = OwnerAllocator::with_next(u64::MAX);
        assert_eq!(wrapping.allocate().unwrap().id(), u64::MAX);
        assert_eq!(wrapping.allocate().unwrap().id(), 1);
        assert_ne!(wrapping.allocate().unwrap().id(), 0);
    }

    #[test]
    fn owner_allocator_skips_live_owner_across_wrap_and_reuses_only_after_release() {
        let allocator = OwnerAllocator::with_next_and_live(u64::MAX, &[1]);
        assert_eq!(allocator.allocate().unwrap().id(), u64::MAX);
        let wrapped = allocator.allocate().unwrap();
        assert_eq!(wrapped.id(), 2);
        allocator.release(1);
        allocator.set_next_for_test(1);
        assert_eq!(allocator.allocate().unwrap().id(), 1);
    }

    #[test]
    fn owner_allocator_is_unique_under_concurrency() {
        let allocator = OwnerAllocator::default();
        let owners = Arc::new(Mutex::new(Vec::new()));
        std::thread::scope(|scope| {
            for _ in 0..16 {
                let allocator = allocator.clone();
                let owners = owners.clone();
                scope.spawn(move || {
                    let mut local = Vec::with_capacity(1_000);
                    for _ in 0..1_000 {
                        local.push(allocator.allocate().unwrap().id());
                    }
                    owners.lock().unwrap().extend(local);
                });
            }
        });
        let owners = owners.lock().unwrap();
        assert_eq!(owners.len(), 16_000);
        assert_eq!(
            owners.iter().copied().collect::<HashSet<_>>().len(),
            owners.len()
        );
        assert!(!owners.contains(&0));
    }

    #[test]
    fn owner_lease_releases_live_owner_idempotently_on_drop() {
        let allocator = OwnerAllocator::default();
        let lease = allocator.allocate().expect("owner lease");
        assert_eq!(allocator.live_count(), 1);
        assert_eq!(lease.id(), 1);
        lease.release();
        lease.release();
        assert_eq!(allocator.live_count(), 0);
    }

    #[test]
    fn owner_lease_can_be_transferred_without_releasing_until_guard_drop() {
        let allocator = OwnerAllocator::default();
        let lease = allocator.allocate().expect("owner lease");
        let owner = lease;
        assert_eq!(allocator.live_count(), 1);
        drop(owner);
        assert_eq!(allocator.live_count(), 0);
    }

    #[test]
    fn cleanup_connection_releases_lease_before_drop() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = calls.clone();
        let dispatcher = EngineDispatcher::with_cleanup(
            Arc::new(|_| Box::pin(async { Ok(json!({"ok": true})) })),
            Arc::new(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
            }),
        );
        let lease = dispatcher.allocate_owner().unwrap();
        dispatcher.cleanup_connection_sync(&lease);
        dispatcher.cleanup_connection_sync(&lease);
        assert_eq!(dispatcher.live_owner_count(), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn validates_wire_request_into_typed_dispatch_request() {
        let request = DispatchRequest::from_wire(json!({
            "_req_id": "req-1",
            "operation": "ark.read",
            "params": {"id": "object-1"}
        }))
        .expect("valid request");

        assert_eq!(request.request_id.as_deref(), Some("req-1"));
        assert_eq!(request.operation, Operation::Named("ark.read".into()));
        assert_eq!(request.params, json!({"params": {"id": "object-1"}}));
    }

    #[test]
    fn rejects_malformed_and_unknown_shape_at_the_boundary() {
        assert_eq!(
            DispatchRequest::from_wire(json!({"operation": " "})),
            Err(DispatchValidationError::InvalidOperation)
        );
        assert_eq!(
            DispatchRequest::from_wire(json!({"operation": 42})),
            Err(DispatchValidationError::InvalidOperation)
        );
        assert_eq!(
            DispatchRequest::from_wire(json!({"operation": "ark.read", "_req_id": 1})),
            Err(DispatchValidationError::InvalidRequestId)
        );
    }

    #[test]
    fn preserves_explicit_params_and_mixed_siblings_as_legacy_payload() {
        let request = DispatchRequest::from_wire(json!({
            "_req_id": "req-1",
            "operation": "commands.invoke",
            "id": "command-id",
            "params": {"nested": true},
            "extra": 42
        }))
        .expect("valid request");

        assert_eq!(request.request_id.as_deref(), Some("req-1"));
        assert_eq!(
            request.params,
            json!({
                "id": "command-id",
                "params": {"nested": true},
                "extra": 42
            })
        );
    }

    fn request(operation: &str, params: Value) -> DispatchRequest {
        DispatchRequest::from_wire(json!({
            "id": "test-1",
            "operation": operation,
            "params": params,
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn dispatcher_covers_read_write_unknown_structured_and_event_paths() {
        let events = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let events_for_handler = events.clone();
        let dispatcher = EngineDispatcher::new(Arc::new(move |request| {
            let events = events_for_handler.clone();
            Box::pin(async move {
                match request.operation.as_str() {
                    "read" => Ok(json!({"ok": true, "value": 7})),
                    "write" => Ok(json!({"ok": true, "written": true})),
                    "event" => {
                        events.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Ok(json!({"ok": true, "event": "changed"}))
                    }
                    "structured-error" => Err(DispatchError::Failed("bad params".into())),
                    _ => Err(DispatchError::Failed("unknown operation".into())),
                }
            })
        }));

        assert_eq!(
            dispatcher
                .dispatch(request("read", json!({})))
                .await
                .unwrap()["value"],
            7
        );
        assert_eq!(
            dispatcher
                .dispatch(request("write", json!({"x": 1})))
                .await
                .unwrap()["written"],
            true
        );
        assert_eq!(
            dispatcher
                .dispatch(request("event", json!({})))
                .await
                .unwrap()["event"],
            "changed"
        );
        assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            dispatcher.dispatch(request("unknown", json!({}))).await,
            Err(DispatchError::Failed("unknown operation".into()))
        );
        assert_eq!(
            dispatcher
                .dispatch(request("structured-error", json!(null)))
                .await,
            Err(DispatchError::Failed("bad params".into()))
        );
    }

    #[tokio::test]
    async fn dispatcher_timeout_and_cancellation_are_bounded_by_caller() {
        let dispatcher = EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                Ok(json!({"ok": true}))
            })
        }));
        let timed = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            dispatcher.dispatch(request("slow", json!({}))),
        )
        .await;
        assert!(timed.is_err());

        let dispatcher_for_task = dispatcher.clone();
        let task = tokio::spawn(async move {
            dispatcher_for_task
                .dispatch(request("slow", json!({})))
                .await
        });
        task.abort();
        assert!(task
            .await
            .expect_err("task must be cancelled")
            .is_cancelled());
    }
}
