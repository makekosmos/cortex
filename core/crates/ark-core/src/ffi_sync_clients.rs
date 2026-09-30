use super::*;

impl ArkCore {
    pub(super) async fn spawn_client(
        &self,
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
            device_id,
            device_name,
            space_id,
            own_addresses,
            auth_secret,
        ));

        let listener = self.listener.read().await.clone();
        let listener_change = listener.clone();
        client
            .set_on_change(Arc::new(move |entity| {
                if let Some(l) = listener_change.as_ref() {
                    if let Ok(json) = serde_json::to_string(&entity) {
                        l.on_entity_changed(json);
                    }
                }
            }))
            .await;
        let listener_connected = listener.clone();
        let server_for_register = server.clone();
        let peer_addresses = peer.addresses.clone();
        client
            .set_on_connected(Arc::new(move |peer_device_id, peer_name| {
                let listener = listener_connected.clone();
                let server = server_for_register.clone();
                let addrs = peer_addresses.clone();
                let peer_device_id_clone = peer_device_id.clone();
                let peer_name_clone = peer_name.clone();
                tokio::spawn(async move {
                    server
                        .register_external_peer(&peer_device_id_clone, &peer_name_clone, addrs)
                        .await;
                    if let Some(l) = listener.as_ref() {
                        l.on_peer_connected(peer_device_id, peer_name);
                    }
                });
            }))
            .await;
        let listener_disconnected = listener;
        let server_for_count = server.clone();
        client
            .set_on_disconnected(Arc::new(move |peer_device_id| {
                let listener = listener_disconnected.clone();
                let server = server_for_count.clone();
                tokio::spawn(async move {
                    if let Some(l) = listener.as_ref() {
                        let remaining = server.connected_peer_count().await as u32;
                        l.on_peer_disconnected(peer_device_id, remaining);
                    }
                });
            }))
            .await;

        client.start();
        clients.lock().await.insert(peer.device_id.clone(), client);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn spawn_seed_client(
        &self,
        server: &Arc<SyncServer>,
        storage: &Arc<SqliteStorageBackend>,
        clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
        addresses: Vec<String>,
        device_id: String,
        device_name: String,
        space_id: String,
        own_addresses: Vec<String>,
        auth_secret: Option<String>,
        sync_bind: SyncBind,
    ) {
        let reachable: Vec<String> = addresses
            .into_iter()
            .filter(|a| !own_addresses.contains(a) && sync_bind.accepts_peer_address(a))
            .collect();
        if reachable.is_empty() {
            return;
        }
        let temp_id = format!("seed-{}", chrono::Utc::now().timestamp_millis());
        let peer = PeerRecord {
            device_id: temp_id,
            device_name: "Bootstrap".to_string(),
            addresses: reachable,
            last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            last_address: None,
        };
        self.spawn_client(
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

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn wire_beacon(
        &self,
        beacon: &Arc<BroadcastDiscovery>,
        server: &Arc<SyncServer>,
        storage: &Arc<SqliteStorageBackend>,
        clients: &Arc<TokioMutex<HashMap<String, Arc<SyncClient>>>>,
        device_id: String,
        device_name: String,
        space_id: String,
        own_addresses: Vec<String>,
        auth_secret: Option<String>,
        sync_bind: SyncBind,
    ) {
        let server = server.clone();
        let storage = storage.clone();
        let clients = clients.clone();
        let listener = self.listener.read().await.clone();
        beacon
            .set_on_peer_discovered(Arc::new(move |peer: BeaconPeer| {
                let server = server.clone();
                let storage = storage.clone();
                let clients = clients.clone();
                let device_id = device_id.clone();
                let device_name = device_name.clone();
                let space_id = space_id.clone();
                let own = own_addresses.clone();
                let auth_secret = auth_secret.clone();
                let listener = listener.clone();
                tokio::spawn(async move {
                    let addrs: Vec<String> = if peer.addresses.is_empty() {
                        vec![peer.address.clone()]
                    } else {
                        peer.addresses.clone()
                    };
                    let reachable: Vec<String> = addrs
                        .into_iter()
                        .filter(|a| !own.contains(a) && sync_bind.accepts_peer_address(a))
                        .collect();
                    if reachable.is_empty() {
                        return;
                    }
                    server
                        .register_external_peer(
                            &peer.device_id,
                            &peer.device_name,
                            reachable.clone(),
                        )
                        .await;

                    if server.is_connected_to(&peer.device_id).await {
                        return;
                    }

                    let existing = clients.lock().await.get(&peer.device_id).cloned();
                    if let Some(existing) = existing {
                        existing
                            .update_peer(PeerRecord {
                                device_id: peer.device_id.clone(),
                                device_name: peer.device_name.clone(),
                                addresses: reachable,
                                last_seen: chrono::Utc::now()
                                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                                last_address: None,
                            })
                            .await;
                        return;
                    }

                    // Instantiate a fresh SyncClient.
                    let client = Arc::new(SyncClient::new(
                        storage.clone() as Arc<dyn StorageBackend>,
                        PeerRecord {
                            device_id: peer.device_id.clone(),
                            device_name: peer.device_name.clone(),
                            addresses: reachable,
                            last_seen: chrono::Utc::now()
                                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            last_address: None,
                        },
                        device_id.clone(),
                        device_name.clone(),
                        space_id.clone(),
                        own.clone(),
                        auth_secret.clone(),
                    ));

                    let listener_change = listener.clone();
                    client
                        .set_on_change(Arc::new(move |entity| {
                            if let Some(l) = listener_change.as_ref() {
                                if let Ok(json) = serde_json::to_string(&entity) {
                                    l.on_entity_changed(json);
                                }
                            }
                        }))
                        .await;
                    let listener_connect = listener.clone();
                    client
                        .set_on_connected(Arc::new(move |peer_device_id, peer_name| {
                            if let Some(l) = listener_connect.as_ref() {
                                l.on_peer_connected(peer_device_id, peer_name);
                            }
                        }))
                        .await;
                    let listener_disconnect = listener.clone();
                    client
                        .set_on_disconnected(Arc::new(move |peer_device_id| {
                            if let Some(l) = listener_disconnect.as_ref() {
                                l.on_peer_disconnected(peer_device_id, 0);
                            }
                        }))
                        .await;
                    client.start();
                    clients.lock().await.insert(peer.device_id, client);
                });
            }))
            .await;
    }
}

// Helper so async methods can take an owning snapshot of the runtime
// without holding the TokioMutex guard.
impl SyncRuntime {
    pub(super) fn clone_refs(&self) -> CloneSyncRuntime {
        CloneSyncRuntime {
            server: self.server.clone(),
            storage: self.storage.clone(),
            clients: self.clients.clone(),
            relay: self.relay.clone(),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            space_id: self.space_id.clone(),
            auth_secret: self.auth_secret.clone(),
            own_addresses: self.own_addresses.clone(),
            bind: self.bind,
        }
    }
}
