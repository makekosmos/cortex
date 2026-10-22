use super::*;

/// Wires the standard event-stream callbacks (`entity_changed`,
/// `peer_connected`, `peer_disconnected`) onto a `RelaySync` instance.
/// Shared between the relay and iroh branches of `handle_start_sync` — the
/// orchestration layer (`RelaySync`) is transport-neutral, so the same
/// callback wiring applies regardless of which `SyncTransport` drives it.
pub(super) async fn wire_relay_sync_events(relay_sync: &Arc<RelaySync>) {
    relay_sync
        .set_on_change(Arc::new(|entity| {
            // Incoming peer-write notification. `entity_changed` — общее событие
            // (raw consumers). Но renderer-подписчики (Eden) слушают типизированные
            // `object_upserted`/`object_deleted` — те же, что эмитит локальная
            // запись (`Request::UpsertObject`/`DeleteObject`). Без этого входящий
            // sync долетает в БД, но UI не перерисовывается до перезагрузки.
            emit_event(json!({
                "event": "entity_changed",
                "entity": entity,
            }));
            if entity.entity_type == "object" {
                if entity.deleted == Some(true) {
                    emit_event(json!({
                        "event": "object_deleted",
                        "id": entity.id,
                    }));
                } else {
                    emit_event(json!({
                        "event": "object_upserted",
                        "id": entity.id,
                        "type_id": entity.data.get("type_id").cloned().unwrap_or(Value::Null),
                    }));
                }
            }
        }))
        .await;
    relay_sync
        .set_on_peer_connect(Arc::new(|peer_device_id| {
            emit_event(json!({
                "event": "peer_connected",
                "device_id": peer_device_id,
            }));
        }))
        .await;
    relay_sync
        .set_on_peer_disconnect(Arc::new(|peer_device_id, remaining| {
            emit_event(json!({
                "event": "peer_disconnected",
                "device_id": peer_device_id,
                "remaining": remaining,
            }));
        }))
        .await;
    relay_sync
        .set_on_pairing_changed(Arc::new(|| {
            emit_event(json!({"event": "pairing_changed"}));
        }))
        .await;
}

pub(super) async fn wire_relay_registry_fanout(
    relay_sync: &Arc<RelaySync>,
    server: &Arc<SyncServer>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<DirectClient>>>>,
) {
    let server = Arc::downgrade(server);
    let clients = Arc::downgrade(clients);
    relay_sync
        .set_on_registry_events(Arc::new(move |events| {
            let server = server.clone();
            let clients = clients.clone();
            crate::task_set::spawn(async move {
                if let Some(server) = server.upgrade() {
                    for event in &events {
                        server.broadcast_registry_event(event.clone()).await;
                    }
                }
                let Some(clients) = clients.upgrade() else {
                    return;
                };
                let peers = clients.lock().await.values().cloned().collect::<Vec<_>>();
                for client in peers {
                    for event in &events {
                        client.broadcast_registry_event(event.clone()).await;
                    }
                }
            });
        }))
        .await;
}

pub(super) async fn wire_server_registry_fanout(
    server: &Arc<SyncServer>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<DirectClient>>>>,
    relay_sync: Option<&Arc<RelaySync>>,
) {
    let clients = Arc::downgrade(clients);
    let relay = relay_sync.map(Arc::downgrade);
    server
        .set_on_registry_events(Arc::new(move |events| {
            let clients = clients.clone();
            let relay = relay.clone();
            crate::task_set::spawn(async move {
                if let Some(clients) = clients.upgrade() {
                    let peers = clients.lock().await.values().cloned().collect::<Vec<_>>();
                    for client in peers {
                        for event in &events {
                            client.broadcast_registry_event(event.clone()).await;
                        }
                    }
                }
                if let Some(relay) = relay.and_then(|relay| relay.upgrade()) {
                    for event in &events {
                        if let Err(error) = relay.publish_registry_event(event.clone()).await {
                            eprintln!("registry event relay fanout failed: {error}");
                        }
                    }
                }
            });
        }))
        .await;
}
