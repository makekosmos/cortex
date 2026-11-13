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

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchPhase {
    Started,
    Finished,
}

#[cfg(any(test, feature = "test-support"))]
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
    /// The caller's process image (queried by its same-user PID, never
    /// self-declared) is the resolved Manager executable. This is the
    /// Manager's trust level — equivalent to the WS desktop-authority lease
    /// for ops a trusted local UI may trigger (`system.privileged.enable`),
    /// without widening `desktop_authorized` to also cover `grant.*`,
    /// consents and package snapshots.
    pub manager_process: bool,
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
    #[cfg(any(test, feature = "test-support"))]
    observer: Arc<Mutex<Option<DispatchObserver>>>,
}

impl EngineDispatcher {
    pub fn new(handler: DispatchHandler) -> Self {
        Self {
            handler,
            cleanup: Arc::new(|_| {}),
            owners: OwnerAllocator::default(),
            #[cfg(any(test, feature = "test-support"))]
            observer: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_cleanup(handler: DispatchHandler, cleanup: CleanupHandler) -> Self {
        Self {
            handler,
            cleanup,
            owners: OwnerAllocator::default(),
            #[cfg(any(test, feature = "test-support"))]
            observer: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn dispatch(&self, request: DispatchRequest) -> DispatchResult {
        #[cfg(any(test, feature = "test-support"))]
        let observer = self
            .observer
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        #[cfg(any(test, feature = "test-support"))]
        if let Some(observer) = &observer {
            observer(
                DispatchPhase::Started,
                request.client.connection_id.unwrap_or_default(),
                request.operation.as_str(),
            );
        }
        #[cfg(any(test, feature = "test-support"))]
        let owner_id = request.client.connection_id.unwrap_or_default();
        #[cfg(any(test, feature = "test-support"))]
        let operation = request.operation.as_str().to_string();
        let result = (self.handler)(request).await;
        #[cfg(any(test, feature = "test-support"))]
        if let Some(observer) = &observer {
            observer(DispatchPhase::Finished, owner_id, &operation);
        }
        result
    }

    #[cfg(any(test, feature = "test-support"))]
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
#[path = "engine_dispatch/tests.rs"]
mod tests;
