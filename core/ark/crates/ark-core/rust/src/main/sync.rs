// ---------------------------------------------------------------------------
// Sync handlers
// ---------------------------------------------------------------------------

/// Wires the standard event-stream callbacks (`entity_changed`,
/// `peer_connected`, `peer_disconnected`) onto a `RelaySync` instance.
/// Shared between the relay and iroh branches of `handle_start_sync` — the
/// orchestration layer (`RelaySync`) is transport-neutral, so the same
/// callback wiring applies regardless of which `SyncTransport` drives it.
async fn wire_relay_sync_events(relay_sync: &Arc<RelaySync>) {
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
}

#[allow(clippy::too_many_arguments)]
async fn handle_start_sync(
    space_id: String,
    device_id: String,
    device_name: Option<String>,
    port: Option<u16>,
    seed_addresses: Option<Vec<String>>,
    relay_url: Option<String>,
    relay_api_key: Option<String>,
    auth_secret: Option<String>,
    use_iroh: bool,
    iroh_peer_ticket: Option<String>,
) -> Result<Value, String> {
    // Idempotency: tear down any running runtime first.
    handle_stop_sync().await;

    let device_name = device_name.unwrap_or_else(get_host_device_name);
    let ws_port = port.unwrap_or(LAN_SYNC_PORT);

    let shared_conn = get_shared_conn()?;
    let storage = Arc::new(SqliteStorageBackend::new(shared_conn.clone()));
    storage.set_device_id(&device_id)?;

    let server = Arc::new(SyncServer::new(storage.clone() as Arc<dyn StorageBackend>));
    server.set_auth_secret(auth_secret.clone()).await;

    // Wire server callbacks -> event stream
    {
        let device_id_for_cb = device_id.clone();
        server
            .set_on_change(Arc::new(move |entity| {
                emit_event(json!({
                    "event": "entity_changed",
                    "entity": entity,
                }));
                let _ = device_id_for_cb; // silence unused capture lint
            }))
            .await;
    }
    {
        server
            .set_on_peer_connect(Arc::new(move |peer_device_id| {
                emit_event(json!({
                    "event": "peer_connected",
                    "device_id": peer_device_id,
                }));
            }))
            .await;
    }
    {
        server
            .set_on_peer_disconnect(Arc::new(move |peer_device_id, remaining| {
                emit_event(json!({
                    "event": "peer_disconnected",
                    "device_id": peer_device_id,
                    "remaining": remaining,
                }));
            }))
            .await;
    }

    let own_addresses: Vec<String> = get_own_addresses(ws_port)
        .into_iter()
        .filter(|a| is_address_routable(a))
        .collect();
    let own_addresses_shared = Arc::new(TokioMutex::new(own_addresses.clone()));

    server
        .start_with_addr(
            &space_id,
            &device_id,
            Some(&device_name),
            Some(own_addresses.clone()),
            &format!("0.0.0.0:{ws_port}"),
        )
        .await?;

    let transport_choice = select_transport(use_iroh, &relay_url);
    #[allow(unused_mut, unused_assignments)]
    let mut iroh_our_ticket: Option<String> = None;
    // `iroh_peer_ticket` is only read inside the `#[cfg(feature = "iroh-spike")]`
    // branch below; reference it here so a no-feature build doesn't warn about
    // an unused parameter (the field itself must stay on the wire schema
    // regardless of build per the UniFFI/JSON-RPC surface-stability rule).
    let _ = &iroh_peer_ticket;

    let transport_state = Some(transport_choice.clone());
    let start_params = SyncStartParams {
        space_id: space_id.clone(),
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        port,
        seed_addresses: seed_addresses.clone(),
        relay_url: relay_url.clone(),
        relay_api_key: relay_api_key.clone(),
        auth_secret: auth_secret.clone(),
        use_iroh,
        iroh_peer_ticket: iroh_peer_ticket.clone(),
    };
    let relay = if transport_choice == TransportChoice::Relay {
        let relay_url = relay_url.clone().expect("Relay choice implies relay_url");
        let transport: Arc<dyn ark_core::sync_transport::SyncTransport> =
            Arc::new(ark_core::relay_transport::RelayTransport::new(
                ark_core::relay_transport::RelayConfig {
                    url: relay_url.clone(),
                    space_id: space_id.clone(),
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    api_key: relay_api_key.clone().unwrap_or_default(),
                    auth_secret: auth_secret.clone(),
                },
            ));
        let relay_sync = RelaySync::with_transport(
            storage.clone() as Arc<dyn StorageBackend>,
            RelaySyncConfig {
                relay_url,
                relay_api_key: relay_api_key.clone(),
                space_id: space_id.clone(),
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                auth_secret: auth_secret.clone(),
            },
            transport,
        );
        wire_relay_sync_events(&relay_sync).await;
        relay_sync.start().await?;
        Some(relay_sync)
    } else if transport_choice == TransportChoice::Iroh {
        #[cfg(feature = "iroh-spike")]
        {
            let secret_key = {
                let conn = shared_conn.lock().unwrap_or_else(|e| e.into_inner());
                ark_core::iroh_transport::load_or_generate_secret_key(&conn)
                    .map_err(|e| format!("iroh transport: failed to load identity: {e}"))?
            };
            let peer_addr = match iroh_peer_ticket.as_deref() {
                Some(ticket) => Some(ark_core::iroh_transport::from_ticket(ticket)?),
                None => None,
            };
            let iroh_transport = Arc::new(ark_core::iroh_transport::IrohTransport::new(
                ark_core::iroh_transport::IrohConfig {
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    space_id: space_id.clone(),
                    secret_key: Some(secret_key),
                    peer_addr,
                    peer_ticket: iroh_peer_ticket.clone(),
                    relay_mode: None,
                    auth_secret: auth_secret.clone(),
                },
            ));
            // RelaySyncConfig.relay_url is unused by `with_transport` (only
            // `RelaySync::new` reads it to build a `RelayTransport`) — pass an
            // empty string rather than widening the struct for one unused field.
            let relay_sync = RelaySync::with_transport(
                storage.clone() as Arc<dyn StorageBackend>,
                RelaySyncConfig {
                    relay_url: String::new(),
                    relay_api_key: None,
                    space_id: space_id.clone(),
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    auth_secret: auth_secret.clone(),
                },
                iroh_transport.clone() as Arc<dyn ark_core::sync_transport::SyncTransport>,
            );
            wire_relay_sync_events(&relay_sync).await;
            relay_sync.start().await?;
            // `start()` binds the endpoint, so `our_ticket()` is available now.
            // Snapshot it onto `SyncRuntime` for `GetOwnIrohTicket` — capture
            // failures are logged but not fatal (pairing UI degrades to "no
            // ticket yet" rather than aborting an otherwise-successful start).
            iroh_our_ticket = match iroh_transport.our_ticket().await {
                Ok(ticket) => Some(ticket),
                Err(e) => {
                    eprintln!("[handle_start_sync] our_ticket() failed: {e}");
                    None
                }
            };
            Some(relay_sync)
        }
        #[cfg(not(feature = "iroh-spike"))]
        {
            return Err(
                "iroh transport requested (use_iroh) but this build was compiled without the \
                 iroh-spike feature; rebuild with --features iroh-spike or use relay_url instead"
                    .to_string(),
            );
        }
    } else {
        None
    };

    // Start SyncClient connections for every known peer.
    let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let known_peers = server.get_known_peers().await;
    let removed_peer_ids = server.get_removed_peer_ids().await;
    for peer in known_peers {
        if peer.device_id == device_id || removed_peer_ids.iter().any(|id| id == &peer.device_id) {
            continue;
        }
        let reachable: Vec<String> = peer
            .addresses
            .iter()
            .filter(|a| !own_addresses.contains(a))
            .cloned()
            .collect();
        if reachable.is_empty() {
            continue;
        }
        let peer_rec = PeerRecord {
            addresses: reachable,
            ..peer
        };
        spawn_sync_client(
            &server,
            &storage,
            &clients,
            peer_rec,
            device_id.clone(),
            device_name.clone(),
            space_id.clone(),
            own_addresses.clone(),
            auth_secret.clone(),
        )
        .await;
    }

    // Seed addresses from QR payload / initial-join flow.
    if let Some(addrs) = seed_addresses {
        if !addrs.is_empty() {
            start_seed_client(
                &server,
                &storage,
                &clients,
                addrs,
                device_id.clone(),
                device_name.clone(),
                space_id.clone(),
                own_addresses.clone(),
                auth_secret.clone(),
            )
            .await;
        }
    }

    // Start beacon discovery.
    let beacon = Arc::new(BroadcastDiscovery::new());
    let beacon_clone = beacon.clone();
    let server_for_beacon = server.clone();
    let clients_for_beacon = clients.clone();
    let storage_for_beacon = storage.clone();
    let device_id_for_beacon = device_id.clone();
    let device_name_for_beacon = device_name.clone();
    let space_id_for_beacon = space_id.clone();
    let own_addresses_for_beacon = own_addresses.clone();
    let auth_secret_for_beacon = auth_secret.clone();
    beacon
        .set_on_peer_discovered(Arc::new(move |peer: BeaconPeer| {
            // Callbacks from UDP recv run on tokio tasks — but this one is
            // invoked from a sync closure. Spawn to an async context so we
            // can await the storage / server.
            let server = server_for_beacon.clone();
            let clients = clients_for_beacon.clone();
            let storage = storage_for_beacon.clone();
            let device_id = device_id_for_beacon.clone();
            let device_name = device_name_for_beacon.clone();
            let space_id = space_id_for_beacon.clone();
            let own = own_addresses_for_beacon.clone();
            let auth_secret = auth_secret_for_beacon.clone();
            tokio::spawn(async move {
                let addrs: Vec<String> = if peer.addresses.is_empty() {
                    vec![peer.address.clone()]
                } else {
                    peer.addresses.clone()
                };
                let reachable: Vec<String> = addrs
                    .into_iter()
                    .filter(|a| !own.contains(a) && is_address_routable(a))
                    .collect();
                if reachable.is_empty() {
                    return;
                }
                if server
                    .get_removed_peer_ids()
                    .await
                    .iter()
                    .any(|id| id == &peer.device_id)
                {
                    return;
                }
                server
                    .register_external_peer(&peer.device_id, &peer.device_name, reachable.clone())
                    .await;
                emit_event(json!({
                    "event": "peer_list_updated",
                    "peers": server.get_connected_peer_entries().await.iter().map(|(id, name)| {
                        json!({"device_id": id, "device_name": name})
                    }).collect::<Vec<_>>(),
                }));

                if server.is_connected_to(&peer.device_id).await {
                    return;
                }
                let existing = clients.lock().await.get(&peer.device_id).cloned();
                if let Some(existing) = existing {
                    // Update its peer record so reconnect picks the new addresses.
                    existing
                        .update_peer(PeerRecord {
                            device_id: peer.device_id.clone(),
                            device_name: peer.device_name.clone(),
                            addresses: reachable.clone(),
                            last_seen: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            last_address: None,
                        })
                        .await;
                    return;
                }

                let peer_rec = PeerRecord {
                    device_id: peer.device_id.clone(),
                    device_name: peer.device_name.clone(),
                    addresses: reachable,
                    last_seen: chrono::Utc::now()
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    last_address: None,
                };
                spawn_sync_client(
                    &server,
                    &storage,
                    &clients,
                    peer_rec,
                    device_id,
                    device_name,
                    space_id,
                    own,
                    auth_secret,
                )
                .await;
            });
        }))
        .await;

    beacon_clone
        .start(BroadcastDiscoveryOptions {
            space_id: space_id.clone(),
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            ws_port,
        })
        .await?;

    let runtime = SyncRuntime {
        server,
        storage,
        clients,
        relay,
        transport_choice: transport_state,
        start_params,
        iroh_our_ticket,
        beacon: beacon_clone,
        space_id,
        device_id,
        device_name,
        auth_secret,
        own_addresses: own_addresses_shared,
    };
    *SYNC.lock().await = Some(Arc::new(runtime));

    Ok(json!(true))
}

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

