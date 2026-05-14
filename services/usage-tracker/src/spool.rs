// Phase 4 (scaffold): local spool для буферизации usage-tracker writes когда
// Kosmos host недоступен (cold-start race, кратковременный crash).
//
// Реализация — простая in-memory очередь. Persistent SQLite spool — следующий
// шаг (когда понадобится переживать restart usage-tracker'а).
//
// API:
//   - SpoolEntry — запись (operation + params + retry counter)
//   - Spool — in-memory FIFO с max capacity
//   - Spool::push / Spool::drain — атомарные ops

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpoolEntry {
    pub operation: String,
    pub params: serde_json::Value,
    pub queued_at: String,
    pub retry_count: u32,
}

pub struct Spool {
    inner: Mutex<VecDeque<SpoolEntry>>,
    max_entries: usize,
}

impl Spool {
    pub fn new() -> Self {
        Self::with_capacity(MAX_ENTRIES)
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
            max_entries,
        }
    }

    /// Добавить запись. Если очередь переполнена — drop'аем самую старую и
    /// возвращаем true (drop'нули) или false (всё ОК).
    pub fn push(&self, entry: SpoolEntry) -> bool {
        let mut queue = self.inner.lock().unwrap();
        let dropped = if queue.len() >= self.max_entries {
            queue.pop_front();
            true
        } else {
            false
        };
        queue.push_back(entry);
        dropped
    }

    /// Извлечь все накопленные записи (для flush'а при reconnect).
    pub fn drain(&self) -> Vec<SpoolEntry> {
        let mut queue = self.inner.lock().unwrap();
        queue.drain(..).collect()
    }

    /// Извлечь до N записей (если хотим batched flush'и).
    pub fn drain_n(&self, n: usize) -> Vec<SpoolEntry> {
        let mut queue = self.inner.lock().unwrap();
        let take = n.min(queue.len());
        queue.drain(..take).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Spool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_entry(op: &str) -> SpoolEntry {
        SpoolEntry {
            operation: op.to_string(),
            params: json!({}),
            queued_at: "2026-05-13T15:00:00Z".to_string(),
            retry_count: 0,
        }
    }

    #[test]
    fn push_increments_len() {
        let spool = Spool::new();
        assert_eq!(spool.len(), 0);
        let dropped = spool.push(make_entry("upsert_tracked_app"));
        assert!(!dropped);
        assert_eq!(spool.len(), 1);
    }

    #[test]
    fn drain_empties_queue() {
        let spool = Spool::new();
        spool.push(make_entry("a"));
        spool.push(make_entry("b"));
        spool.push(make_entry("c"));
        let entries = spool.drain();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].operation, "a");
        assert_eq!(entries[1].operation, "b");
        assert_eq!(entries[2].operation, "c");
        assert!(spool.is_empty());
    }

    #[test]
    fn drain_n_partial() {
        let spool = Spool::new();
        for i in 0..5 {
            spool.push(make_entry(&format!("op-{i}")));
        }
        let first_batch = spool.drain_n(2);
        assert_eq!(first_batch.len(), 2);
        assert_eq!(spool.len(), 3);
        let rest = spool.drain();
        assert_eq!(rest.len(), 3);
    }

    #[test]
    fn overflow_drops_oldest() {
        let spool = Spool::with_capacity(3);
        spool.push(make_entry("a"));
        spool.push(make_entry("b"));
        spool.push(make_entry("c"));
        let dropped = spool.push(make_entry("d"));
        assert!(dropped, "should signal that oldest was dropped");
        assert_eq!(spool.len(), 3);
        let entries = spool.drain();
        assert_eq!(entries[0].operation, "b"); // 'a' dropped
        assert_eq!(entries[2].operation, "d");
    }

    #[test]
    fn fifo_order() {
        let spool = Spool::new();
        spool.push(make_entry("first"));
        spool.push(make_entry("second"));
        let drained = spool.drain();
        assert_eq!(drained[0].operation, "first");
        assert_eq!(drained[1].operation, "second");
    }
}
