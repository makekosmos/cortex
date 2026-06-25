// Command bus: in-memory registry of "commands" published by connected WS
// clients, used by the global launcher (Kosmos) to display searchable
// open/action items contributed by individual Kepler apps.
//
// Each connected WS client owns a list of `CommandManifest`. On disconnect,
// that client's commands are removed automatically. Invocations are broadcast
// to all subscribers — the owning client filters by `id` on its side.
//
// Operations are intercepted by `ws_server` for the `commands.` namespace
// (register / unregister / list / invoke) so they never reach `ark-core-rpc`.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};

/// Identifier of a WS connection within this process. Assigned by `ws_server`
/// at accept-time (monotonic u64), used as the registration key.
pub type ClientId = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandManifest {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// "open" | "action".
    pub category: String,
}

/// Event emitted to all WS connections subscribed to the command bus.
#[derive(Debug, Clone)]
pub enum CommandBusEvent {
    /// Registry changed (register / unregister / disconnect). Payload — full
    /// flat list of currently-registered commands.
    Changed(Vec<CommandManifest>),
    /// A command invocation request. Owning client filters by `id`; non-owners
    /// ignore. `params` is the JSON value passed to `commands.invoke`.
    Invoked {
        id: String,
        params: serde_json::Value,
    },
}

pub struct CommandBus {
    registrations: Arc<Mutex<HashMap<ClientId, Vec<CommandManifest>>>>,
    events_tx: broadcast::Sender<CommandBusEvent>,
}

impl Default for CommandBus {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBus {
    pub fn new() -> Self {
        let (events_tx, _) = broadcast::channel::<CommandBusEvent>(128);
        Self {
            registrations: Arc::new(Mutex::new(HashMap::new())),
            events_tx,
        }
    }

    /// Append `manifests` to `client_id`'s registration list. Within a single
    /// client, entries are deduped by `id` — last-write-wins (existing entries
    /// with same id are replaced).
    pub async fn register(&self, client_id: ClientId, manifests: Vec<CommandManifest>) {
        let mut guard = self.registrations.lock().await;
        let entry = guard.entry(client_id).or_default();
        for m in manifests {
            if let Some(pos) = entry.iter().position(|e| e.id == m.id) {
                entry[pos] = m;
            } else {
                entry.push(m);
            }
        }
    }

    /// Remove the listed `ids` from `client_id`'s registration list. Ids not
    /// owned by the client are ignored silently.
    pub async fn unregister(&self, client_id: ClientId, ids: &[String]) {
        let mut guard = self.registrations.lock().await;
        if let Some(entry) = guard.get_mut(&client_id) {
            entry.retain(|m| !ids.iter().any(|i| i == &m.id));
            if entry.is_empty() {
                guard.remove(&client_id);
            }
        }
    }

    /// Drop every command owned by `client_id`. Used on WS disconnect.
    pub async fn unregister_all(&self, client_id: ClientId) {
        let mut guard = self.registrations.lock().await;
        guard.remove(&client_id);
    }

    /// Flat snapshot of every currently-registered command across all clients.
    /// Order is not stable across clients (HashMap iteration), but per-client
    /// insertion order is preserved.
    pub async fn list(&self) -> Vec<CommandManifest> {
        let guard = self.registrations.lock().await;
        let mut out = Vec::new();
        for entry in guard.values() {
            out.extend(entry.iter().cloned());
        }
        out
    }

    /// Subscribe to bus events. Returns a fresh receiver — drop to unsubscribe.
    pub fn subscribe(&self) -> broadcast::Receiver<CommandBusEvent> {
        self.events_tx.subscribe()
    }

    /// Broadcast `commands_changed` with the current flat list. Errors when
    /// there are no subscribers are ignored (normal during startup).
    pub async fn broadcast_changed(&self) {
        let snapshot = self.list().await;
        let _ = self.events_tx.send(CommandBusEvent::Changed(snapshot));
    }

    /// Broadcast `command_invoked`. The owning client filters by `id`.
    pub fn broadcast_invoked(&self, id: String, params: serde_json::Value) {
        let _ = self.events_tx.send(CommandBusEvent::Invoked { id, params });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, title: &str, category: &str) -> CommandManifest {
        CommandManifest {
            id: id.to_string(),
            title: title.to_string(),
            subtitle: None,
            category: category.to_string(),
        }
    }

    #[tokio::test]
    async fn register_dedupes_by_id_within_client() {
        let bus = CommandBus::new();
        bus.register(
            1,
            vec![
                manifest("eden.open", "Open Eden", "open"),
                manifest("eden.new", "New Note", "action"),
            ],
        )
        .await;
        // Re-register same id with new title — must replace, not duplicate.
        bus.register(1, vec![manifest("eden.open", "Открыть Eden", "open")])
            .await;

        let all = bus.list().await;
        assert_eq!(all.len(), 2, "dedupe must keep one entry per id");
        let eden_open = all
            .iter()
            .find(|m| m.id == "eden.open")
            .expect("eden.open must remain");
        assert_eq!(eden_open.title, "Открыть Eden", "last-write-wins on title");
        assert!(all.iter().any(|m| m.id == "eden.new"));
    }

    #[tokio::test]
    async fn unregister_all_on_disconnect_removes_only_that_client_commands() {
        let bus = CommandBus::new();
        bus.register(
            1,
            vec![
                manifest("eden.open", "Open Eden", "open"),
                manifest("eden.new", "New Note", "action"),
            ],
        )
        .await;
        bus.register(2, vec![manifest("delphi.open", "Open Delphi", "open")])
            .await;

        bus.unregister_all(1).await;

        let all = bus.list().await;
        assert_eq!(all.len(), 1, "only client 2's commands must remain");
        assert_eq!(all[0].id, "delphi.open");
    }

    #[tokio::test]
    async fn unregister_specific_ids_keeps_other_commands() {
        let bus = CommandBus::new();
        bus.register(
            1,
            vec![
                manifest("a", "A", "open"),
                manifest("b", "B", "open"),
                manifest("c", "C", "action"),
            ],
        )
        .await;

        bus.unregister(1, &["b".to_string()]).await;

        let all = bus.list().await;
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|m| m.id == "a"));
        assert!(all.iter().any(|m| m.id == "c"));
        assert!(!all.iter().any(|m| m.id == "b"));
    }

    #[tokio::test]
    async fn broadcast_changed_delivers_snapshot_to_subscriber() {
        let bus = CommandBus::new();
        let mut rx = bus.subscribe();
        bus.register(1, vec![manifest("x", "X", "open")]).await;
        bus.broadcast_changed().await;

        let evt = rx.recv().await.expect("event must arrive");
        match evt {
            CommandBusEvent::Changed(list) => {
                assert_eq!(list.len(), 1);
                assert_eq!(list[0].id, "x");
            }
            CommandBusEvent::Invoked { .. } => panic!("expected Changed, got Invoked"),
        }
    }

    #[tokio::test]
    async fn broadcast_invoked_delivers_id_and_params() {
        let bus = CommandBus::new();
        let mut rx = bus.subscribe();
        bus.broadcast_invoked(
            "eden.new".to_string(),
            serde_json::json!({ "title": "draft" }),
        );

        let evt = rx.recv().await.expect("event must arrive");
        match evt {
            CommandBusEvent::Invoked { id, params } => {
                assert_eq!(id, "eden.new");
                assert_eq!(params, serde_json::json!({ "title": "draft" }));
            }
            CommandBusEvent::Changed(_) => panic!("expected Invoked, got Changed"),
        }
    }
}
