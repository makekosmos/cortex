//! Library-level event emitter. Originally lived в `bin/main.rs`, но Phase 2
//! schema-drift handling в `db.rs` нуждается в emit'е событий тоже — а `db.rs`
//! часть lib, не binary. Поэтому emitter переехал сюда; binary (`main.rs`)
//! устанавливает sender при старте через `set_event_sender(tx)`.

use std::sync::Mutex;

use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

static EVENT_TX: Mutex<Option<UnboundedSender<Value>>> = Mutex::new(None);

/// Установить отправителя событий. Вызывается из `main.rs` один раз при старте.
pub fn set_event_sender(tx: UnboundedSender<Value>) {
    if let Ok(mut guard) = EVENT_TX.lock() {
        *guard = Some(tx);
    }
}

/// Эмитировать событие. Если sender не установлен (например, библиотека
/// используется out-of-bin context'а — Kepler host напрямую вызывает db), tихо
/// игнорируется.
pub fn emit_event(event: Value) {
    if let Ok(guard) = EVENT_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::mpsc;

    #[test]
    fn emit_with_no_sender_is_noop() {
        // Должно не паниковать, даже если sender ни разу не установлен.
        emit_event(json!({"event": "test"}));
    }

    #[test]
    fn emit_after_set_sender_delivers() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        set_event_sender(tx);
        emit_event(json!({"event": "test", "value": 42}));
        let received = rx.try_recv().expect("event delivered");
        assert_eq!(received["event"], "test");
        assert_eq!(received["value"], 42);
    }
}
