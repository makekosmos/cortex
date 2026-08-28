use super::*;

#[uniffi::export]
impl ArkCore {
    pub fn start_sync(
        &self,
        config: FfiSyncConfig,
        listener: Box<dyn ArkEventListener>,
    ) -> Result<bool> {
        // ---------------------------------------------------------------
        // AC1 fix: Do NOT call block_on on the JNI calling thread.
        //
        // Strategy:
        //  1. Stop any prior sync (via the existing runtime — this is fast).
        //  2. Install the listener and open DB on the current thread (sync ops).
        //  3. Spawn a dedicated OS thread that drives an async setup coroutine
        //     inside the shared tokio runtime.
        //  4. The spawned thread sends back a oneshot result once the server
        //     and beacon are bound.
        //  5. We wait on a std::sync::mpsc::channel for this result — this
        //     blocks the caller only until ports are bound (typically < 500 ms),
        //     NOT for the lifetime of the sync engine.
        // ---------------------------------------------------------------

        // 1. Tear down any prior sync runtime (fast async on our own runtime).
        self.runtime.block_on(async {
            self.stop_sync_inner().await;
        });

        // 2a. Install listener.
        let listener_arc: Arc<dyn ArkEventListener> = Arc::from(listener);
        self.runtime.block_on(async {
            *self.listener.write().await = Some(listener_arc.clone());
        });

        // 2b. Open DB if the caller supplied a path and we haven't opened one yet.
        if let Some(path) = config.db_path.as_ref() {
            if self.db.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                let conn = open_db(path).map_err(ArkCoreError::from)?;
                init_schema(&conn).map_err(ArkCoreError::from)?;
                *self.db.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(Arc::new(StdMutex::new(conn)));
            }
        }

        let shared_conn = self
            .db
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .cloned()
            .ok_or_else(|| err("DB not opened; call open_db first"))?;

        // 3. Prepare a result channel and a shutdown channel.
        let (result_tx, result_rx) =
            std::sync::mpsc::channel::<std::result::Result<SyncRuntime, ArkCoreError>>();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Save the shutdown sender so stop_sync can signal the background thread.
        *self.sync_shutdown.lock().unwrap_or_else(|e| e.into_inner()) = Some(shutdown_tx);

        // Clone data needed inside the background thread.
        let runtime_handle = self.runtime.handle().clone();

        // We need to clone self for the background thread.
        // Use a raw pointer trick safe because ArkCore is Arc<ArkCore> and
        // its lifetime is tied to the JVM-referenced object.
        let self_arc = unsafe {
            // SAFETY: ArkCore is always heap-allocated as Arc<ArkCore>.
            // The caller (Kotlin/UniFFI) holds a reference that outlives
            // this thread's lifetime.
            Arc::increment_strong_count(self as *const ArkCore);
            Arc::from_raw(self as *const ArkCore)
        };

        let config_clone = config.clone();
        let listener_for_thread = listener_arc.clone();

        std::thread::Builder::new()
            .name("ark-sync-setup".to_string())
            .spawn(move || {
                let config = config_clone;
                let result_tx = result_tx;

                let setup_result = runtime_handle.block_on(async {
                    let storage = Arc::new(SqliteStorageBackend::new(shared_conn.clone()));
                    storage
                        .set_device_id(&config.device_id)
                        .map_err(ArkCoreError::from)?;

                    let device_name = config
                        .device_name
                        .clone()
                        .unwrap_or_else(get_host_device_name);
                    let ws_port = config.port.unwrap_or(LAN_SYNC_PORT as u32) as u16;

                    let server =
                        Arc::new(SyncServer::new(storage.clone() as Arc<dyn StorageBackend>));
                    server.set_auth_secret(config.auth_secret.clone()).await;

                    // Wire server → listener (using the self_arc inside the thread).
                    self_arc.install_server_callbacks(&server).await;

                    let own_addresses: Vec<String> = get_own_addresses(ws_port)
                        .into_iter()
                        .filter(|a| is_address_routable(a))
                        .collect();

                    server
                        .start_with_addr(
                            &config.space_id,
                            &config.device_id,
                            Some(&device_name),
                            Some(own_addresses.clone()),
                            &format!("0.0.0.0:{ws_port}"),
                        )
                        .await
                        .map_err(ArkCoreError::from)?;

                    let transport_choice = crate::transport_select::select_transport(
                        config.use_iroh,
                        &config.relay_url,
                    );
                    #[allow(unused_mut, unused_assignments)]
                    let mut iroh_our_ticket: Option<String> = None;

                    let relay = if transport_choice
                        == crate::transport_select::TransportChoice::Relay
                    {
                        let relay_url = config
                            .relay_url
                            .clone()
                            .expect("Relay choice implies relay_url");
                        let relay_sync = RelaySync::new(
                            storage.clone() as Arc<dyn StorageBackend>,
                            RelaySyncConfig {
                                relay_url,
                                relay_api_key: config.relay_api_key.clone(),
                                space_id: config.space_id.clone(),
                                device_id: config.device_id.clone(),
                                device_name: device_name.clone(),
                                auth_secret: config.auth_secret.clone(),
                            },
                        );
                        self_arc.install_relay_callbacks(&relay_sync).await;
                        relay_sync.start().await.map_err(ArkCoreError::from)?;
                        Some(relay_sync)
                    } else if transport_choice == crate::transport_select::TransportChoice::Iroh {
                        #[cfg(feature = "iroh-spike")]
                        {
                            let secret_key = {
                                let conn = shared_conn.lock().unwrap_or_else(|e| e.into_inner());
                                crate::iroh_transport::load_or_generate_secret_key(&conn)
                                    .map_err(ArkCoreError::from)?
                            };
                            let peer_addr = match config.iroh_peer_ticket.as_deref() {
                                Some(ticket) => Some(
                                    crate::iroh_transport::from_ticket(ticket)
                                        .map_err(ArkCoreError::from)?,
                                ),
                                None => None,
                            };
                            let iroh_transport =
                                Arc::new(crate::iroh_transport::IrohTransport::new(
                                    crate::iroh_transport::IrohConfig {
                                        device_id: config.device_id.clone(),
                                        device_name: device_name.clone(),
                                        space_id: config.space_id.clone(),
                                        secret_key: Some(secret_key),
                                        peer_addr,
                                        peer_ticket: config.iroh_peer_ticket.clone(),
                                        relay_mode: None,
                                        auth_secret: config.auth_secret.clone(),
                                    },
                                ));
                            let relay_sync = RelaySync::with_transport(
                                storage.clone() as Arc<dyn StorageBackend>,
                                RelaySyncConfig {
                                    relay_url: String::new(),
                                    relay_api_key: None,
                                    space_id: config.space_id.clone(),
                                    device_id: config.device_id.clone(),
                                    device_name: device_name.clone(),
                                    auth_secret: config.auth_secret.clone(),
                                },
                                iroh_transport.clone()
                                    as Arc<dyn crate::sync_transport::SyncTransport>,
                            );
                            self_arc.install_relay_callbacks(&relay_sync).await;
                            relay_sync.start().await.map_err(ArkCoreError::from)?;
                            iroh_our_ticket = match iroh_transport.our_ticket().await {
                                Ok(ticket) => Some(ticket),
                                Err(e) => {
                                    eprintln!("[ArkCore::start_sync] our_ticket() failed: {e}");
                                    None
                                }
                            };
                            Some(relay_sync)
                        }
                        #[cfg(not(feature = "iroh-spike"))]
                        {
                            return Err(err(
                                "iroh transport requested (use_iroh) but this build was \
                                 compiled without the iroh-spike feature; rebuild with \
                                 --features iroh-spike or use relay_url instead",
                            ));
                        }
                    } else {
                        None
                    };

                    // Start clients for known peers.
                    let clients: Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>> =
                        Arc::new(TokioMutex::new(HashMap::new()));
                    let known = server.get_known_peers().await;
                    for peer in known {
                        if peer.device_id == config.device_id {
                            continue;
                        }
                        let reachable: Vec<String> = peer
                            .addresses
                            .iter()
                            .filter(|a| !own_addresses.contains(a) && is_address_routable(a))
                            .cloned()
                            .collect();
                        if reachable.is_empty() {
                            continue;
                        }
                        let peer_rec = PeerRecord {
                            addresses: reachable,
                            ..peer
                        };
                        self_arc
                            .spawn_client(
                                &server,
                                &storage,
                                &clients,
                                peer_rec,
                                config.device_id.clone(),
                                device_name.clone(),
                                config.space_id.clone(),
                                own_addresses.clone(),
                                config.auth_secret.clone(),
                            )
                            .await;
                    }

                    // Seed addresses (QR bootstrap).
                    if !config.seed_addresses.is_empty() {
                        self_arc
                            .spawn_seed_client(
                                &server,
                                &storage,
                                &clients,
                                config.seed_addresses.clone(),
                                config.device_id.clone(),
                                device_name.clone(),
                                config.space_id.clone(),
                                own_addresses.clone(),
                                config.auth_secret.clone(),
                            )
                            .await;
                    }

                    // Beacon.
                    let beacon = Arc::new(BroadcastDiscovery::new());
                    self_arc
                        .wire_beacon(
                            &beacon,
                            &server,
                            &storage,
                            &clients,
                            config.device_id.clone(),
                            device_name.clone(),
                            config.space_id.clone(),
                            own_addresses.clone(),
                            config.auth_secret.clone(),
                        )
                        .await;
                    beacon
                        .start(BroadcastDiscoveryOptions {
                            space_id: config.space_id.clone(),
                            device_id: config.device_id.clone(),
                            device_name: device_name.clone(),
                            ws_port,
                        })
                        .await
                        .map_err(ArkCoreError::from)?;

                    // Publish listener via self_arc so later helpers can see it.
                    *self_arc.listener.write().await = Some(listener_for_thread);

                    let sync_runtime = SyncRuntime {
                        server,
                        storage,
                        clients,
                        relay,
                        iroh_our_ticket,
                        beacon,
                        device_id: config.device_id.clone(),
                        device_name,
                        space_id: config.space_id.clone(),
                        auth_secret: config.auth_secret.clone(),
                        own_addresses,
                    };

                    Ok::<SyncRuntime, ArkCoreError>(sync_runtime)
                });

                // 4. Send the result back to the caller.
                let _ = result_tx.send(setup_result);

                // 5. Keep the thread alive so tokio tasks spawned inside the
                //    setup (beacon, server, etc.) continue running.
                // We wait for the shutdown signal.
                let _ = runtime_handle.block_on(shutdown_rx);

                // Drop self_arc to release the Arc reference.
                drop(self_arc);
            })
            .map_err(|e| err(format!("thread spawn failed: {e}")))?;

        // 5. Wait for the setup thread to report success/failure.
        let sync_runtime = result_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| err("start_sync timed out (>10 s)"))??;

        // Store the runtime handle so broadcast_change_json / get_connected_peers work.
        self.runtime.block_on(async {
            *self.sync.lock().await = Some(sync_runtime);
        });

        Ok(true)
    }
}
