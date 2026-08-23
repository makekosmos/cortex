//! Clock abstraction for production and deterministic tests.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub trait Clock: Send + Sync + 'static {
    fn now_ms(&self) -> u64;
}

#[derive(Debug, Default, Clone)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct MockClock {
    inner: Arc<AtomicU64>,
}

impl MockClock {
    pub fn new(initial_ms: u64) -> Self {
        Self {
            inner: Arc::new(AtomicU64::new(initial_ms)),
        }
    }

    pub fn advance(&self, ms: u64) {
        self.inner.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn set(&self, ms: u64) {
        self.inner.store(ms, Ordering::SeqCst);
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Clock for MockClock {
    fn now_ms(&self) -> u64 {
        self.inner.load(Ordering::SeqCst)
    }
}
