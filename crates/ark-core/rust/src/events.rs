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
    use tokio::sync::mpsc::{self, error::TryRecvError};

    #[test]
    fn emit_with_no_sender_is_noop() {
        // Должно не паниковать, даже если sender ни разу не установлен
        // (или установлен другим concurrent-тестом — emit просто уйдёт в его
        // channel и тихо дропнется).
        emit_event(json!({"event": "emit_with_no_sender_is_noop"}));
    }

    #[test]
    fn emit_after_set_sender_delivers() {
        // EVENT_TX — process-wide static; cargo test бегает параллельно, и
        // другие тесты в крейте (db.rs ops) тоже могут emit'ить — их события
        // попадут в наш rx. Фильтруем по уникальному tag'у, а в конце сбрасываем
        // sender, чтобы leftover-эмиты от следующих тестов не висели в памяти.
        const TAG: &str = "events_rs_delivers_unique_tag";

        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        set_event_sender(tx);
        emit_event(json!({"event": TAG, "value": 42}));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let received = loop {
            match rx.try_recv() {
                Ok(v) if v["event"] == TAG => break v,
                Ok(_) => continue,
                Err(TryRecvError::Empty) => {
                    if std::time::Instant::now() > deadline {
                        panic!("event with tag {TAG} not delivered within 2s");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(TryRecvError::Disconnected) => unreachable!("our tx is held in EVENT_TX"),
            }
        };
        assert_eq!(received["value"], 42);

        // Cleanup: следующие тесты не должны лить в нашу (про rx будет drop) channel.
        if let Ok(mut g) = EVENT_TX.lock() {
            *g = None;
        }
    }
}
