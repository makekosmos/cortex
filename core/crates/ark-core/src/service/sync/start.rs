use super::*;
use crate::db::{CommandContext, EnsureUsageSequenceMigratedCommand, SetLocalSyncKeyValueCommand};
use crate::service::runtime::get_db_handle;

pub(crate) async fn handle_start_sync(
    state: &Arc<ServiceState>,
    params: StartSyncParams,
) -> Result<Value, String> {
    let StartSyncParams {
        space_id,
        device_id,
        device_name,
        port,
        seed_addresses,
        relay_url,
        relay_api_key,
        auth_secret,
        use_iroh,
        iroh_peer_ticket,
        iroh_relay_url,
        pairing_connect,
        discovery_enabled,
        bind,
        app_version,
    } = params;
    // Idempotency: tear down any running runtime first.
    handle_stop_sync(state).await;
    ark_net::host::set_app_version(app_version.clone());

    let device_name = device_name.unwrap_or_else(get_host_device_name);
    // Port 0 would bind an ephemeral port while advertising ":0" to peers.
    let ws_port = match port {
        Some(0) => return Err("invalid sync port 0; expected 1..=65535".to_string()),
        Some(p) => p,
        None => LAN_SYNC_PORT,
    };

    let db_handle = get_db_handle(state)?;
    db_handle
        .submit(
            CommandContext::engine(),
            EnsureUsageSequenceMigratedCommand::new(device_id.clone()),
        )
        .await
        .map_err(|error| error.to_string())?;
    db_handle
        .submit(
            CommandContext::engine(),
            SetLocalSyncKeyValueCommand::new(
                crate::db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.into(),
                device_id.clone(),
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
    let storage = Arc::new(SqliteStorageBackend::with_db_handle(db_handle.clone()));
    storage.set_device_id(&device_id)?;

    let server = Arc::new(SyncServer::new(
        storage.clone() as Arc<dyn SyncRuntimeStorage>
    ));
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

    // The bind choice resolves every address the stack opens and advertises:
    // loopback mode must not claim LAN addresses it is not listening on.
    let own_addresses: Vec<String> = bind.own_addresses(ws_port);
    let own_addresses_shared = Arc::new(TokioMutex::new(own_addresses.clone()));

    // Fail before opening listeners or starting transport tasks if persisted
    // peer state is corrupt or unavailable. This keeps a failed start retryable.
    let known_peers = server.get_known_peers().await?;
    let removed_peer_ids = server.get_removed_peer_ids().await?;

    server
        .start_with_addr(
            &space_id,
            &device_id,
            Some(&device_name),
            Some(own_addresses.clone()),
            &bind.ws_bind_addr(ws_port),
        )
        .await?;

    let transport_choice = select_transport(use_iroh, &relay_url);
    let transport_state = Some(transport_choice.clone());
    // Our iroh ticket is published through this channel: `our_ticket()`
    // waits on relay homing (network), which must not stall the sequential
    // request worker — the iroh branch fills it from a spawned task.
    let (iroh_our_ticket_tx, iroh_our_ticket) = watch::channel(None);
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
        iroh_relay_url: iroh_relay_url.clone(),
        // One-shot: later restarts derive from these params, and a stored
        // peer key must be what re-admits the endpoint — not a replayed
        // pairing intent.
        pairing_connect: false,
        discovery_enabled,
        bind,
        app_version,
    };
    let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let relay = if transport_choice == TransportChoice::Relay {
        let relay_url = relay_url.clone().expect("Relay choice implies relay_url");
        let transport: Arc<dyn crate::sync_transport::SyncTransport> = Arc::new(
            crate::relay_transport::RelayTransport::new(crate::relay_transport::RelayConfig {
                url: relay_url.clone(),
                space_id: space_id.clone(),
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                api_key: relay_api_key.clone().unwrap_or_default(),
                auth_secret: auth_secret.clone(),
            }),
        );
        let relay_sync = RelaySync::with_transport(
            storage.clone() as Arc<dyn SyncRuntimeStorage>,
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
        let secret_key = db_handle
            .submit(
                CommandContext::engine(),
                crate::db::LoadOrGenerateIrohSecretKeyCommand,
            )
            .await
            .map_err(|error| format!("iroh transport: failed to load identity: {error}"))?;
        let peer_addr = match iroh_peer_ticket.as_deref() {
            Some(ticket) => Some(crate::iroh_transport::from_ticket(ticket)?),
            None => None,
        };
        let peer_endpoint = peer_addr.as_ref().map(|addr| addr.id.to_string());
        // A loopback-bound endpoint cannot be reached through a
        // relay anyway, and tests must stay offline; RelayMode::Disabled
        // also makes `our_ticket()` advertise the loopback socket.
        // An explicit `iroh_relay_url` overrides both: self-hosted
        // relays, and tests that need an unreachable relay.
        let iroh_relay_mode = match &iroh_relay_url {
            Some(url) => {
                let relay_url: iroh::RelayUrl = url
                    .parse()
                    .map_err(|e| format!("invalid iroh_relay_url {url:?}: {e}"))?;
                Some(iroh::RelayMode::custom([relay_url]))
            }
            None => match bind {
                SyncBind::Loopback => Some(iroh::RelayMode::Disabled),
                SyncBind::AllInterfaces => None,
            },
        };
        // `our_ticket()` only waits on the network while homing on a
        // relay; with relays disabled it resolves from local addresses.
        let ticket_is_offline = matches!(iroh_relay_mode, Some(iroh::RelayMode::Disabled));
        let iroh_transport = Arc::new(crate::iroh_transport::IrohTransport::new(
            crate::iroh_transport::IrohConfig {
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                space_id: space_id.clone(),
                secret_key: Some(secret_key),
                peer_addr,
                peer_ticket: iroh_peer_ticket.clone(),
                relay_mode: iroh_relay_mode,
                auth_secret: auth_secret.clone(),
                bind,
            },
        ));
        // RelaySyncConfig.relay_url is unused by `with_transport` (only
        // `RelaySync::new` reads it to build a `RelayTransport`) — pass an
        // empty string rather than widening the struct for one unused field.
        let relay_sync = RelaySync::with_transport(
            storage.clone() as Arc<dyn SyncRuntimeStorage>,
            RelaySyncConfig {
                relay_url: String::new(),
                relay_api_key: None,
                space_id: space_id.clone(),
                device_id: device_id.clone(),
                device_name: device_name.clone(),
                auth_secret: auth_secret.clone(),
            },
            iroh_transport.clone() as Arc<dyn crate::sync_transport::SyncTransport>,
        );
        wire_relay_sync_events(&relay_sync).await;
        // Only a `connect_with_pairing_code` start is a pairing intent
        // (`pairing_connect`); boot restores replay the stored ticket for an
        // already-paired endpoint, whose Hello passes on the stored key —
        // no outgoing attempt must appear in the snapshot. Set it *before*
        // the transport starts dialing, or the responder's first Hello
        // would be parked as an unsolicited request.
        if pairing_connect {
            if let Some(endpoint) = peer_endpoint {
                relay_sync.begin_outgoing_pairing(endpoint).await;
            }
        }
        relay_sync.start().await?;
        // `start()` binds the endpoint, so `our_ticket()` can be computed.
        // It may wait on relay homing (network, up to its own timeout), so
        // it runs on a spawned task — the worker must stay free for local
        // ops while the ticket is pending. The ticket lands in the
        // `iroh_our_ticket` watch channel on `SyncRuntime`; capture
        // failures are logged but not fatal (pairing UI degrades to "no
        // ticket yet" rather than aborting an otherwise-successful start).
        // `get_sync_snapshot`'s `pairing_available` flips to true once the
        // channel fills.
        if ticket_is_offline {
            // Loopback/tests: the ticket is local, fill it before
            // `start_sync` returns so passive reads never observe a
            // transient `null` here.
            match iroh_transport.our_ticket().await {
                Ok(ticket) => {
                    let _ = iroh_our_ticket_tx.send(Some(ticket));
                }
                Err(e) => {
                    eprintln!("[handle_start_sync] our_ticket() failed: {e}");
                }
            }
        } else {
            let ticket_transport = iroh_transport.clone();
            let ticket_tx = iroh_our_ticket_tx;
            // Tracked by the transport, not detached: this task holds the
            // transport, and the transport holds the ark.db connection, so
            // `stop()` must abort and join it (KOS-369).
            let ticket_task = tokio::spawn(async move {
                match ticket_transport.our_ticket().await {
                    Ok(ticket) => {
                        let _ = ticket_tx.send(Some(ticket));
                        emit_event(json!({"event": "pairing_changed"}));
                    }
                    Err(e) => {
                        eprintln!("[handle_start_sync] our_ticket() failed: {e}");
                    }
                }
            });
            iroh_transport.track(ticket_task);
        }
        Some(relay_sync)
    } else {
        None
    };
    if let Some(relay_sync) = relay.as_ref() {
        wire_relay_registry_fanout(relay_sync, &server, &clients).await;
    }
    wire_server_registry_fanout(&server, &clients, relay.as_ref()).await;

    // Start SyncClient connections for every known peer.
    let client_env = ClientEnv {
        server: server.clone(),
        storage: storage.clone(),
        clients: clients.clone(),
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        space_id: space_id.clone(),
        own_addresses: own_addresses.clone(),
        auth_secret: auth_secret.clone(),
        relay: relay.clone(),
    };
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
        spawn_sync_client(&client_env, peer_rec).await;
    }

    // Seed addresses from QR payload / initial-join flow.
    if let Some(addrs) = seed_addresses {
        if !addrs.is_empty() {
            start_seed_client(&client_env, addrs, bind).await;
        }
    }

    // Start beacon discovery unless explicitly disabled for a ticket-paired
    // transport that must not bind the shared LAN beacon port. Loopback mode
    // never binds the beacon: UDP broadcasts do not traverse loopback, so the
    // socket could only trigger a firewall prompt without ever seeing a peer.
    let beacon = Arc::new(BroadcastDiscovery::new());
    if discovery_enabled && bind.discovery_supported() {
        let beacon_clone = beacon.clone();
        let server_for_beacon = server.clone();
        let clients_for_beacon = clients.clone();
        let storage_for_beacon = storage.clone();
        let device_id_for_beacon = device_id.clone();
        let device_name_for_beacon = device_name.clone();
        let space_id_for_beacon = space_id.clone();
        let own_addresses_for_beacon = own_addresses.clone();
        let auth_secret_for_beacon = auth_secret.clone();
        let relay_for_beacon = relay.clone();
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
                let relay = relay_for_beacon.clone();
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
                    let removed_peer_ids = match server.get_removed_peer_ids().await {
                        Ok(ids) => ids,
                        Err(error) => {
                            eprintln!(
                                "[handle_start_sync] failed to load blocked peer list: {error}"
                            );
                            return;
                        }
                    };
                    if removed_peer_ids.iter().any(|id| id == &peer.device_id) {
                        return;
                    }
                    if let Err(error) = server
                        .register_external_peer(PeerRecord {
                            device_id: peer.device_id.clone(),
                            device_name: peer.device_name.clone(),
                            addresses: reachable.clone(),
                            last_seen: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            last_address: None,
                            platform: None,
                            app_version: None,
                        })
                        .await
                    {
                        eprintln!(
                            "[handle_start_sync] failed to register discovered peer: {error}"
                        );
                        return;
                    }
                    emit_event(json!({
                        "event": "peer_list_updated",
                        "peers": server.get_connected_peer_entries().await.iter().map(|entry| {
                            json!({
                                "device_id": entry.device_id,
                                "device_name": entry.device_name,
                                "platform": entry.platform,
                                "app_version": entry.app_version,
                            })
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
                                platform: None,
                                app_version: None,
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
                        platform: None,
                        app_version: None,
                    };
                    spawn_sync_client(
                        &ClientEnv {
                            server: server.clone(),
                            storage: storage.clone(),
                            clients: clients.clone(),
                            device_id,
                            device_name,
                            space_id,
                            own_addresses: own,
                            auth_secret,
                            relay,
                        },
                        peer_rec,
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
    *state.sync.lock().await = Some(Arc::new(runtime));

    Ok(json!(true))
}
