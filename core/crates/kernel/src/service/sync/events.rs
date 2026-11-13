use super::*;

/// Entities carry their object in camelCase (`typeId`); older snapshots in the
/// wild still hold `type_id`, so accept both.
fn object_type_id(data: &serde_json::Map<String, Value>) -> Option<&str> {
    data.get("typeId")
        .or_else(|| data.get("type_id"))
        .and_then(Value::as_str)
}

/// Wires the standard event-stream callbacks (`entity_changed`,
/// `peer_connected`, `peer_disconnected`) onto a `SyncSession` instance.
/// Shared between the relay and iroh branches of `handle_start_sync` — the
/// orchestration layer (`SyncSession`) is transport-neutral, so the same
/// One applied inbound entity, announced on the same bus the local write paths
/// use. `entity_changed` stays the catch-all; renderer subscribers listen to
/// the typed events, so an inbound peer write repaints without a reload.
pub(crate) fn announce_inbound_change(entity: SyncEntity) {
    emit_event(json!({
        "event": "entity_changed",
        "entity": entity,
    }));
    if entity.entity_type == "object" {
        let op = if entity.deleted == Some(true) {
            emit_event(json!({
                "event": "object_deleted",
                "id": entity.id,
            }));
            "delete"
        } else {
            emit_event(json!({
                "event": "object_upserted",
                "id": entity.id,
                "type_id": object_type_id(&entity.data),
            }));
            "upsert"
        };
        emit_event(crate::events::objects_changed(
            &entity.id,
            object_type_id(&entity.data),
            op,
        ));
    } else if entity.entity_type == "object_link" {
        emit_event(crate::events::links_changed(
            &entity,
            if entity.deleted == Some(true) {
                "delete"
            } else {
                "upsert"
            },
        ));
    }
}

/// callback wiring applies regardless of which `SyncTransport` drives it.
pub(super) async fn wire_sync_session_events(sync_session: &Arc<SyncSession>) {
    sync_session
        .set_on_change(Arc::new(announce_inbound_change))
        .await;
    sync_session
        .set_on_peer_connect(Arc::new(|peer_device_id| {
            emit_event(json!({
                "event": "peer_connected",
                "device_id": peer_device_id,
            }));
        }))
        .await;
    sync_session
        .set_on_peer_disconnect(Arc::new(|peer_device_id, remaining| {
            emit_event(json!({
                "event": "peer_disconnected",
                "device_id": peer_device_id,
                "remaining": remaining,
            }));
        }))
        .await;
    sync_session
        .set_on_pairing_changed(Arc::new(|| {
            emit_event(json!({"event": "pairing_changed"}));
        }))
        .await;
}

pub(super) async fn wire_relay_registry_fanout(
    sync_session: &Arc<SyncSession>,
    server: &Arc<SyncServer>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<DirectClient>>>>,
) {
    let server = Arc::downgrade(server);
    let clients = Arc::downgrade(clients);
    sync_session
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
    sync_session: Option<&Arc<SyncSession>>,
) {
    let clients = Arc::downgrade(clients);
    let relay = sync_session.map(Arc::downgrade);
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
