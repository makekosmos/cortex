use super::*;

pub(super) async fn handle_start_sync(
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
    discovery_enabled: bool,
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
        discovery_enabled,
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

    // Start beacon discovery unless explicitly disabled for a ticket-paired
    // transport that must not bind the shared LAN beacon port.
    let beacon = Arc::new(BroadcastDiscovery::new());
    if discovery_enabled {
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
    }

    let runtime = SyncRuntime {
        server,
        storage,
        clients,
        relay,
        transport_choice: transport_state,
        start_params,
        iroh_our_ticket,
        beacon,
        space_id,
        device_id,
        device_name,
        auth_secret,
        own_addresses: own_addresses_shared,
    };
    *SYNC.lock().await = Some(Arc::new(runtime));

    Ok(json!(true))
}
