//! Clock абстракция — production использует SystemClock над
//! `std::time::SystemTime`, tests используют MockClock с manual `advance`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub trait Clock: Send + Sync + 'static {
    /// Текущее unix-время в миллисекундах.
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

/// Thread-safe MockClock с atomic counter — позволяет advance() из любого
/// потока (нужно для tests где tick() в tokio task).
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
