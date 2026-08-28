// ---------------------------------------------------------------------------
// Sync handlers
// ---------------------------------------------------------------------------

#[path = "sync/start.rs"]
mod sync_start;
#[path = "sync/events.rs"]
mod sync_events;
use sync_events::wire_relay_sync_events;
use sync_start::handle_start_sync;

#[allow(clippy::too_many_arguments)]
async fn spawn_sync_client(
    server: &Arc<SyncServer>,
    storage: &Arc<SqliteStorageBackend>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    peer: PeerRecord,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
) {
    if peer.device_id == device_id {
        return;
    }

    let client = Arc::new(SyncClient::new(
        storage.clone() as Arc<dyn StorageBackend>,
        peer.clone(),
        device_id.clone(),
        device_name.clone(),
        space_id.clone(),
        own_addresses.clone(),
        auth_secret,
    ));

    client
        .set_on_change(Arc::new(|entity| {
            emit_event(json!({
                "event": "entity_changed",
                "entity": entity,
            }));
        }))
        .await;

    let server_for_connect = server.clone();
    let peer_addrs = peer.addresses.clone();
    client
        .set_on_connected(Arc::new(move |peer_device_id, peer_name| {
            let server = server_for_connect.clone();
            let addrs = peer_addrs.clone();
            let peer_device_id_clone = peer_device_id.clone();
            let peer_name_clone = peer_name.clone();
            tokio::spawn(async move {
                server
                    .register_external_peer(&peer_device_id_clone, &peer_name_clone, addrs)
                    .await;
            });
            emit_event(json!({
                "event": "peer_connected",
                "device_id": peer_device_id,
                "device_name": peer_name,
            }));
        }))
        .await;

    let server_for_disconnect = server.clone();
    client
        .set_on_disconnected(Arc::new(move |peer_device_id| {
            let server = server_for_disconnect.clone();
            let peer_device_id_clone = peer_device_id.clone();
            tokio::spawn(async move {
                let remaining = server.connected_peer_count().await;
                emit_event(json!({
                    "event": "peer_disconnected",
                    "device_id": peer_device_id_clone,
                    "remaining": remaining,
                }));
            });
        }))
        .await;

    client.start();
    clients.lock().await.insert(peer.device_id.clone(), client);
}

#[allow(clippy::too_many_arguments)]
async fn start_seed_client(
    server: &Arc<SyncServer>,
    storage: &Arc<SqliteStorageBackend>,
    clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
    addresses: Vec<String>,
    device_id: String,
    device_name: String,
    space_id: String,
    own_addresses: Vec<String>,
    auth_secret: Option<String>,
) {
    let reachable: Vec<String> = addresses
        .into_iter()
        .filter(|a| !own_addresses.contains(a) && is_address_routable(a))
        .collect();
    if reachable.is_empty() {
        return;
    }
    let temp_id = format!("seed-{}", chrono::Utc::now().timestamp_millis());
    let peer = PeerRecord {
        device_id: temp_id.clone(),
        device_name: "Bootstrap".to_string(),
        addresses: reachable,
        last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        last_address: None,
    };
    spawn_sync_client(
        server,
        storage,
        clients,
        peer,
        device_id,
        device_name,
        space_id,
        own_addresses,
        auth_secret,
    )
    .await;
}

async fn handle_stop_sync() {
    let runtime = {
        let mut guard = SYNC.lock().await;
        guard.take()
    };
    if let Some(runtime) = runtime {
        runtime.beacon.stop().await;
        if let Some(relay) = runtime.relay.as_ref() {
            relay.stop();
        }
        let clients: Vec<Arc<SyncClient>> = {
            let clients = runtime.clients.lock().await;
            clients.values().cloned().collect()
        };
        for client in clients {
            client.disconnect().await;
        }
        runtime.server.stop().await;
    }
}

async fn handle_start_sync_with_params(params: SyncStartParams) -> Result<Value, String> {
    handle_start_sync(
        params.space_id,
        params.device_id,
        Some(params.device_name),
        params.port,
        params.seed_addresses,
        params.relay_url,
        params.relay_api_key,
        params.auth_secret,
        params.use_iroh,
        params.iroh_peer_ticket,
    )
    .await
}

fn build_pairing_restart_params(runtime: &SyncRuntime, pairing_code: &str) -> SyncStartParams {
    let mut params = runtime.start_params.clone();
    params.use_iroh = true;
    params.iroh_peer_ticket = Some(pairing_code.trim().to_string());
    params
}

async fn handle_broadcast_change(mut entity: SyncEntity) -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };

    // Stamp HLC on the outgoing entity.
    let hlc = runtime.server.update_entity_hlc(&entity.id).await;
    entity.hlc = hlc;

    // Persist locally so future version-vector exchanges reflect it.
    runtime.storage.apply_entity(&entity).await?;

    // Broadcast via the inbound server sessions.
    runtime
        .server
        .broadcast_live_change(entity.clone(), None)
        .await;

    // Broadcast via outbound clients.
    let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
    for client in clients {
        client.broadcast_live_change(entity.clone()).await;
    }
    if let Some(relay) = runtime.relay.as_ref() {
        relay.broadcast_live_change(entity.clone())?;
    }
    Ok(json!(true))
}

/// Step 4a: surface our iroh pairing ticket for the runtime/UI. `null` when
/// sync isn't running or the running runtime didn't select iroh (relay/no
/// transport, or a build without `iroh-spike`).
async fn handle_get_own_iroh_ticket() -> Result<Value, String> {
    let guard = SYNC.lock().await;
    let ticket = guard
        .as_ref()
        .and_then(|runtime| runtime.iroh_our_ticket.clone());
    Ok(json!(ticket))
}

async fn handle_get_sync_snapshot() -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => {
                return Ok(json!({
                    "running": false,
                    "transport": "unknown",
                    "pairing_available": false,
                    "own_pairing_code_available": false,
                    "peers": [],
                }))
            }
        }
    };

    let mut connected = runtime.server.get_connected_peer_entries().await;
    if let Some(relay) = runtime.relay.as_ref() {
        connected.extend(relay.get_connected_peer_entries().await);
    }
    let known = runtime.server.get_known_peers().await;
    let connected_ids: std::collections::HashSet<String> =
        connected.iter().map(|(id, _)| id.clone()).collect();
    let mut represented_ids = std::collections::HashSet::new();
    let mut peers: Vec<Value> = known
        .into_iter()
        .filter(|peer| peer.device_id != runtime.device_id)
        .map(|peer| {
            represented_ids.insert(peer.device_id.clone());
            let status = if connected_ids.contains(&peer.device_id) {
                "online"
            } else {
                "offline"
            };
            json!({
                "device_id": peer.device_id,
                "device_name": peer.device_name,
                "last_seen": peer.last_seen,
                "status": status,
            })
        })
        .collect();
    peers.extend(
        connected
            .into_iter()
            .filter(|(device_id, _)| {
                device_id != &runtime.device_id && !represented_ids.contains(device_id)
            })
            .map(|(device_id, device_name)| {
                json!({
                    "device_id": device_id,
                    "device_name": device_name,
                    "last_seen": chrono::Utc::now().to_rfc3339(),
                    "status": "online",
                })
            }),
    );

    Ok(json!({
        "running": true,
        "transport": match runtime.transport_choice {
            Some(TransportChoice::Iroh) => "iroh",
            Some(TransportChoice::Relay) => "relay",
            Some(TransportChoice::None) => "lan",
            None => "unknown",
        },
        "pairing_available": runtime.iroh_our_ticket.is_some(),
        "own_pairing_code_available": runtime.iroh_our_ticket.is_some(),
        "peers": peers,
        "local_device": {
            "device_id": runtime.device_id.clone(),
            "device_name": runtime.device_name.clone(),
        },
    }))
}

async fn handle_disconnect_peer(device_id: String) -> Result<Value, String> {
    let guard = SYNC.lock().await;
    let runtime = match guard.as_ref() {
        Some(r) => r.clone(),
        None => return Err("Sync not running".to_string()),
    };
    drop(guard);
    let device_id = device_id.trim();
    if device_id.is_empty() {
        return Err("device_id is empty".to_string());
    }

    let _ = runtime.server.disconnect_peer(device_id).await;

    let client_entries: Vec<(String, Arc<SyncClient>)> = {
        let clients = runtime.clients.lock().await;
        clients
            .iter()
            .map(|(key, client)| (key.clone(), client.clone()))
            .collect()
    };
    let mut removed_client_ids: Vec<String> = Vec::new();
    for (client_key, client) in client_entries {
        let peer = client.current_peer().await;
        if peer.device_id == device_id {
            client.disconnect().await;
            removed_client_ids.push(client_key);
        }
    }
    if !removed_client_ids.is_empty() {
        let mut clients = runtime.clients.lock().await;
        for client_id in removed_client_ids {
            clients.remove(&client_id);
        }
    }

    let remaining = runtime.server.connected_peer_count().await;
    emit_event(json!({
        "event": "peer_disconnected",
        "device_id": device_id,
        "remaining": remaining,
    }));
    emit_event(json!({"event": "peer_list_updated"}));
    Ok(json!(true))
}

async fn handle_connect_with_pairing_code(pairing_code: String) -> Result<Value, String> {
    let code = pairing_code.trim();
    if code.is_empty() {
        return Err("pairing code is empty".to_string());
    }

    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };

    let restart_params = build_pairing_restart_params(&runtime, code);
    let restore_params = runtime.start_params.clone();

    handle_stop_sync().await;
    match handle_start_sync_with_params(restart_params).await {
        Ok(result) => Ok(result),
        Err(err) => {
            if let Err(restore_err) = handle_start_sync_with_params(restore_params).await {
                return Err(format!(
                    "connect_with_pairing_code failed: {err}; restoring previous sync also failed: {restore_err}"
                ));
            }
            Err(format!(
                "connect_with_pairing_code failed: {err}; previous sync restored"
            ))
        }
    }
}

async fn handle_get_connected_peers() -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Ok(json!([])),
        }
    };
    let entries = runtime.server.get_connected_peer_entries().await;

    // Merge in outbound-connected clients (not yet visible in the server peer
    // table if the connection is outbound-only).
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (id, name) in entries {
        if !seen.contains_key(&id) {
            order.push(id.clone());
        }
        seen.insert(id, name);
    }
    let clients: Vec<_> = runtime
        .clients
        .lock()
        .await
        .iter()
        .map(|(device_id, client)| (device_id.clone(), client.clone()))
        .collect();
    for (device_id, client) in clients {
        if seen.contains_key(&device_id) {
            continue;
        }
        let peer = client.current_peer().await;
        if !peer.device_id.is_empty() {
            order.push(peer.device_id.clone());
            seen.insert(peer.device_id, peer.device_name);
        }
    }

    if let Some(relay) = runtime.relay.as_ref() {
        for (device_id, device_name) in relay.get_connected_peer_entries().await {
            if seen.contains_key(&device_id) {
                continue;
            }
            order.push(device_id.clone());
            seen.insert(device_id, device_name);
        }
    }

    let list: Vec<Value> = order
        .into_iter()
        .filter_map(|id| {
            seen.remove(&id).map(|name| {
                json!({
                    "device_id": id,
                    "device_name": name,
                })
            })
        })
        .collect();
    Ok(json!(list))
}

async fn handle_add_seed_peer(addresses: Vec<String>) -> Result<Value, String> {
    let runtime = {
        let guard = SYNC.lock().await;
        match guard.as_ref() {
            Some(r) => r.clone(),
            None => return Err("Sync not running".to_string()),
        }
    };
    let own = runtime.own_addresses.lock().await.clone();
    start_seed_client(
        &runtime.server,
        &runtime.storage,
        &runtime.clients,
        addresses,
        runtime.device_id.clone(),
        runtime.device_name.clone(),
        runtime.space_id.clone(),
        own,
        runtime.auth_secret.clone(),
    )
    .await;
    Ok(json!(true))
}
