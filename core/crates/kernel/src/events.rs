//! Library-level event bus. Originally the sidecar binary wired an mpsc
//! sender into stdout via `set_event_sender`; with ARK hosted in-process the
//! bus is a process-wide `broadcast` channel. `ArkService::subscribe` hands
//! receivers to embedders; `emit_event` is also called from lib modules
//! (notably `db::apply_entity_blocking` for schema-drift sync_error /
//! sync_replay events) and from the background `db_backup` thread.
//!
//! The bus is shared by every `ArkService` instance in the process — the
//! Engine hosts exactly one; multi-instance callers (tests) see all events.

use std::sync::OnceLock;

use crate::types::{ObjectLink, SyncEntity};
use serde_json::{json, Value};
use tokio::sync::broadcast;

static EVENT_BUS: OnceLock<broadcast::Sender<Value>> = OnceLock::new();

fn bus() -> &'static broadcast::Sender<Value> {
    EVENT_BUS.get_or_init(|| broadcast::channel(1024).0)
}

/// Эмитировать событие. Без подписчиков (или когда bus полон) событие тихо
/// игнорируется.
pub fn emit_event(event: Value) {
    let _ = bus().send(event);
}

/// Подписаться на поток событий. Лагнувший подписчик получает
/// `RecvError::Lagged` и должен перечитать состояние.
pub fn subscribe() -> broadcast::Receiver<Value> {
    bus().subscribe()
}

/// A link write names the link, its ends and how it changed. Links carry no
/// type of their own, so this is a separate event rather than an
/// `objects.changed` with an invented type id.
pub(crate) fn links_changed_from(link: &ObjectLink, op: &str) -> Value {
    json!({
        "event": "links.changed",
        "id": link.id,
        "sourceId": link.source_object_id,
        "targetId": link.target_object_id,
        "linkType": link.link_type,
        "op": op,
    })
}

/// Same event, from a sync entity a link command returned.
pub(crate) fn links_changed(entity: &SyncEntity, op: &str) -> Value {
    json!({
        "event": "links.changed",
        "id": entity.id,
        "sourceId": entity.data.get("sourceObjectId").cloned(),
        "targetId": entity.data.get("targetObjectId").cloned(),
        "linkType": entity.data.get("linkType").cloned(),
        "op": op,
    })
}

/// One event for a UI that only wants to know what changed and how: which
/// object, of which type, touched how. `object_upserted`/`object_deleted` stay
/// for the listeners that already read them (engine-mobile's `ChangeListener`).
pub(crate) fn objects_changed(id: &str, type_id: Option<&str>, op: &str) -> Value {
    json!({
        "event": "objects.changed",
        "id": id,
        "typeId": type_id,
        "op": op,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn emit_with_no_subscribers_is_noop() {
        emit_event(json!({"event": "emit_with_no_subscribers_is_noop"}));
    }

    #[test]
    fn emit_reaches_subscriber() {
        const TAG: &str = "events_rs_delivers_unique_tag";
        let mut rx = subscribe();
        emit_event(json!({"event": TAG, "value": 42}));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match rx.try_recv() {
                Ok(v) if v["event"] == TAG => {
                    assert_eq!(v["value"], 42);
                    return;
                }
                Ok(_) | Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Empty) => {
                    assert!(
                        std::time::Instant::now() <= deadline,
                        "event with tag {TAG} not delivered within 2s"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    panic!("event bus is process-wide and never closes")
                }
            }
        }
    }
}
