use super::*;

pub(crate) async fn handle_disconnect_peer(
    state: &Arc<ServiceState>,
    device_id: String,
) -> Result<Value, String> {
    let guard = state.sync.lock().await;
    let runtime = match guard.as_ref() {
        Some(runtime) => runtime.clone(),
        None => return Err("Sync not running".into()),
    };
    drop(guard);
    let device_id = device_id.trim();
    if device_id.is_empty() {
        return Err("device_id is empty".into());
    }

    let mut errors = Vec::new();
    if let Err(error) = runtime.server.disconnect_peer(device_id).await {
        errors.push(format!("server peer teardown: {error}"));
    }
    if let Some(relay) = runtime.relay.as_ref() {
        if let Err(error) = relay.disconnect_peer(device_id).await {
            errors.push(format!("relay peer teardown: {error}"));
        }
    }

    let clients: Vec<(String, Arc<DirectClient>)> = runtime
        .clients
        .lock()
        .await
        .iter()
        .map(|(key, client)| (key.clone(), client.clone()))
        .collect();
    let mut removed = Vec::new();
    for (key, client) in clients {
        if client.current_peer().await.device_id == device_id {
            client.disconnect().await;
            removed.push(key);
        }
    }
    if !removed.is_empty() {
        let mut clients = runtime.clients.lock().await;
        for key in removed {
            clients.remove(&key);
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "peer disconnected with persistence/transport errors: {}",
            errors.join("; ")
        ));
    }

    let mut remaining = runtime.server.connected_peer_count().await;
    if let Some(relay) = runtime.relay.as_ref() {
        remaining += relay.connected_peer_count().await;
    }
    emit_event(json!({
        "event": "peer_disconnected",
        "device_id": device_id,
        "remaining": remaining,
    }));
    emit_event(json!({"event": "peer_list_updated"}));
    Ok(json!(true))
}
