use super::*;

impl SyncServer {
    pub async fn send_signed_integration_frame(
        &self,
        recipient_device_id: &str,
        frame: crate::integration_replication::SignedSyncEnvelope,
    ) -> Result<(), String> {
        let expected_space_id = self.space_id.read().await.clone();
        let expected_origin_node_id = self.device_id.read().await.clone();
        if frame.space_id != expected_space_id {
            return Err("signed integration frame has the wrong space".into());
        }
        if frame.origin_node_id != expected_origin_node_id {
            return Err("signed integration frame origin is not this node".into());
        }
        if frame.recipient_node_id != recipient_device_id {
            return Err("signed integration frame is addressed to another peer".into());
        }
        self.storage
            .validate_outbound_signed_integration_frame(
                &frame,
                &expected_space_id,
                &expected_origin_node_id,
            )
            .await?;
        let peers = self.peers.lock().await;
        let Some(peer) = peers
            .values()
            .find(|peer| peer.authenticated && peer.device_id == recipient_device_id)
        else {
            return Err("target LAN peer is not authenticated".into());
        };
        peer.tx
            .send(Message::Text(serialize_message(
                &LanSyncMessage::SignedIntegrationFrame { frame },
            )))
            .map_err(|_| "LAN peer connection is closed".into())
    }
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            space_id: Arc::new(RwLock::new(String::new())),
            device_id: Arc::new(RwLock::new(String::new())),
            device_name: Arc::new(RwLock::new("ArkSync".to_string())),
            own_addresses: Arc::new(RwLock::new(Vec::new())),
            auth_secret: Arc::new(RwLock::new(None)),
            peers: Arc::new(Mutex::new(HashMap::new())),
            known_peer_records: Arc::new(Mutex::new(Vec::new())),
            shutdown_tx: Arc::new(Mutex::new(None)),
            on_change: Arc::new(Mutex::new(None)),
            on_peer_connect: Arc::new(Mutex::new(None)),
            on_peer_disconnect: Arc::new(Mutex::new(None)),
            on_new_peer_discovered: Arc::new(Mutex::new(None)),
            next_peer_id: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub async fn set_on_change(&self, handler: OnChangeCallback) {
        *self.on_change.lock().await = Some(handler);
    }

    pub async fn set_on_peer_connect(&self, handler: OnPeerConnectCallback) {
        *self.on_peer_connect.lock().await = Some(handler);
    }

    pub async fn set_on_peer_disconnect(&self, handler: OnPeerDisconnectCallback) {
        *self.on_peer_disconnect.lock().await = Some(handler);
    }

    pub async fn set_on_new_peer_discovered(&self, handler: OnNewPeerDiscoveredCallback) {
        *self.on_new_peer_discovered.lock().await = Some(handler);
    }

    pub async fn connected_peer_count(&self) -> usize {
        let peers = self.peers.lock().await;
        peers.values().filter(|p| p.authenticated).count()
    }

    pub async fn get_known_peers(&self) -> Vec<PeerRecord> {
        self.known_peer_records.lock().await.clone()
    }

    pub async fn get_removed_peer_ids(&self) -> Vec<String> {
        load_removed_peer_ids(&self.storage).await
    }

    pub async fn block_peer(&self, device_id: &str) {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return;
        }
        let mut removed = load_removed_peer_ids(&self.storage).await;
        if !removed.iter().any(|id| id == device_id) {
            removed.push(device_id.to_string());
            save_removed_peer_ids(&self.storage, &removed).await;
        }
        let mut known = self.known_peer_records.lock().await;
        let before = known.len();
        known.retain(|peer| peer.device_id != device_id);
        if known.len() != before {
            save_known_peers(&self.storage, &known).await;
        }
    }

    /// Block a peer and explicitly tear down any live inbound sessions for it.
    /// Returns the number of active sessions that were closed.
    pub async fn disconnect_peer(&self, device_id: &str) -> usize {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return 0;
        }
        self.block_peer(device_id).await;

        let mut peers = self.peers.lock().await;
        let peer_ids: Vec<usize> = peers
            .iter()
            .filter_map(|(peer_id, peer)| {
                if peer.device_id == device_id {
                    Some(*peer_id)
                } else {
                    None
                }
            })
            .collect();

        let mut closed = 0usize;
        for peer_id in &peer_ids {
            if let Some(peer) = peers.get_mut(peer_id) {
                if peer.authenticated {
                    peer.authenticated = false;
                    let _ = peer.tx.send(Message::Close(None));
                    closed += 1;
                }
            }
        }
        for peer_id in peer_ids {
            peers.remove(&peer_id);
        }
        closed
    }

    pub(super) async fn is_peer_blocked(&self, device_id: &str) -> bool {
        load_removed_peer_ids(&self.storage)
            .await
            .iter()
            .any(|id| id == device_id)
    }

    /// Dedup connected peers by device_id. Matches the `getConnectedPeerEntries`
    /// semantics of the TS sync server. Order: LinkedHashMap insertion order.
    pub async fn get_connected_peer_entries(&self) -> Vec<(String, String)> {
        let peers = self.peers.lock().await;
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for peer in peers.values() {
            if !peer.authenticated {
                continue;
            }
            if peer.device_id.is_empty() {
                continue;
            }
            if !seen.contains_key(&peer.device_id) {
                order.push(peer.device_id.clone());
            }
            seen.insert(peer.device_id.clone(), peer.device_name.clone());
        }
        order
            .into_iter()
            .map(|id| {
                let name = seen.remove(&id).unwrap_or_default();
                (id, name)
            })
            .collect()
    }

    /// True if any authenticated session matches the given `device_id`.
    pub async fn is_connected_to(&self, device_id: &str) -> bool {
        let peers = self.peers.lock().await;
        peers
            .values()
            .any(|p| p.authenticated && p.device_id == device_id)
    }

    /// Record an externally-connected peer (e.g. a `SyncClient` we just dialled
    /// out to) so `get_known_peers` reflects the merged set. Mirrors the
    /// TS `registerExternalPeer` helper.
    pub async fn register_external_peer(
        &self,
        device_id: &str,
        device_name: &str,
        addresses: Vec<String>,
    ) {
        let my_device_id = self.device_id.read().await.clone();
        let my_addresses = self.own_addresses.read().await.clone();

        if device_id == my_device_id {
            return;
        }
        if !addresses.is_empty() && addresses.iter().all(|a| my_addresses.contains(a)) {
            return;
        }

        if self.is_peer_blocked(device_id).await {
            return;
        }
        let new_record = PeerRecord {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            addresses,
            last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            last_address: None,
        };
        let mut known = self.known_peer_records.lock().await;
        *known = merge_peer_records(&known, &[new_record]);
        save_known_peers(&self.storage, &known).await;
    }

    /// Replace the current own-addresses snapshot. Used when the host network
    /// changes between `start_sync` calls or when an embedder wants to refresh
    /// the routable set.
    pub async fn set_own_addresses(&self, addresses: Vec<String>) {
        *self.own_addresses.write().await = addresses;
    }

    pub async fn set_auth_secret(&self, auth_secret: Option<String>) {
        *self.auth_secret.write().await = normalize_auth_secret(auth_secret);
    }

    pub async fn get_device_id(&self) -> String {
        self.device_id.read().await.clone()
    }

    pub async fn get_device_name(&self) -> String {
        self.device_name.read().await.clone()
    }

    /// Start the WS server on the default LAN sync port.
    pub async fn start(
        &self,
        space_id: &str,
        device_id: &str,
        device_name: Option<&str>,
        own_addresses: Option<Vec<String>>,
    ) -> Result<(), String> {
        self.start_with_addr(
            space_id,
            device_id,
            device_name,
            own_addresses,
            &format!("0.0.0.0:{LAN_SYNC_PORT}"),
        )
        .await
    }

    /// Start the WS server on an explicit bind address. Used by integration
    /// tests that need to avoid port conflicts and by embedders that want a
    /// loopback-only listener.
    pub async fn start_with_addr(
        &self,
        space_id: &str,
        device_id: &str,
        device_name: Option<&str>,
        own_addresses: Option<Vec<String>>,
        bind_addr: &str,
    ) -> Result<(), String> {
        *self.space_id.write().await = space_id.to_string();
        *self.device_id.write().await = device_id.to_string();
        if let Some(name) = device_name {
            *self.device_name.write().await = name.to_string();
        }
        if let Some(addrs) = own_addresses {
            *self.own_addresses.write().await = addrs;
        }

        // Load known peers and evict stale self-references.
        self.load_known_peers().await;
        self.purge_self_peers().await;

        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| format!("Failed to bind {bind_addr}: {e}"))?;

        eprintln!("{TAG} Server listening on {bind_addr}");

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let peers = self.peers.clone();
        let storage = self.storage.clone();
        let space_id = self.space_id.clone();
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let own_addresses = self.own_addresses.clone();
        let auth_secret = self.auth_secret.clone();
        let known_peer_records = self.known_peer_records.clone();
        let on_change = self.on_change.clone();
        let on_peer_connect = self.on_peer_connect.clone();
        let on_peer_disconnect = self.on_peer_disconnect.clone();
        let on_new_peer_discovered = self.on_new_peer_discovered.clone();
        let next_peer_id = self.next_peer_id.clone();

        // Ping task
        let peers_ping = peers.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(PING_INTERVAL_MS));
            loop {
                ticker.tick().await;
                let peers_guard = peers_ping.lock().await;
                for peer in peers_guard.values() {
                    if peer.authenticated {
                        let _ = peer.tx.send(Message::Ping(vec![]));
                    }
                }
            }
        });

        // Accept loop
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _)) => {
                                let peer_id = next_peer_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let ws_stream = match accept_async(stream).await {
                                    Ok(ws) => ws,
                                    Err(e) => {
                                        eprintln!("{TAG} WS accept error: {e}");
                                        continue;
                                    }
                                };

                                let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();
                                let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                                // Writer task
                                tokio::spawn(async move {
                                    while let Some(msg) = rx.recv().await {
                                        if ws_sink.send(msg).await.is_err() {
                                            break;
                                        }
                                    }
                                });

                                let peer_state = PeerState {
                                    device_id: String::new(),
                                    device_name: String::new(),
                                    addresses: Vec::new(),
                                    authenticated: false,
                                    sync_complete: false,
                                    queued_live_changes: Vec::new(),
                                    tx: tx.clone(),
                                };
                                peers.lock().await.insert(peer_id, peer_state);

                                // Reader task
                                let peers_reader = peers.clone();
                                let storage_reader = storage.clone();
                                let space_id_reader = space_id.clone();
                                let device_id_reader = device_id.clone();
                                let device_name_reader = device_name.clone();
                                let own_addresses_reader = own_addresses.clone();
                                let auth_secret_reader = auth_secret.clone();
                                let known_peer_records_reader = known_peer_records.clone();
                                let on_change_reader = on_change.clone();
                                let on_peer_connect_reader = on_peer_connect.clone();
                                let on_peer_disconnect_reader = on_peer_disconnect.clone();
                                let on_new_peer_discovered_reader = on_new_peer_discovered.clone();

                                tokio::spawn(async move {
                                    while let Some(Ok(msg)) = ws_stream_rx.next().await {
                                        match msg {
                                            Message::Text(text) => {
                                                if let Some(sync_msg) = deserialize_message(&text) {
                                sync_server_messages::handle_message(
                                                        peer_id,
                                                        sync_msg,
                                                        &peers_reader,
                                                        &storage_reader,
                                                        &space_id_reader,
                                                        &device_id_reader,
                                                        &device_name_reader,
                                                        &own_addresses_reader,
                                                        &auth_secret_reader,
                                                        &known_peer_records_reader,
                                                        &on_change_reader,
                                                        &on_peer_connect_reader,
                                                        &on_new_peer_discovered_reader,
                                                    ).await;
                                                }
                                            }
                                            Message::Close(_) => break,
                                            _ => {}
                                        }
                                    }

                                    // Peer disconnected
                                    let mut peers_guard = peers_reader.lock().await;
                                    if let Some(peer) = peers_guard.remove(&peer_id) {
                                        if peer.authenticated {
                                            eprintln!("{TAG} Peer disconnected: {} ({})", peer.device_name, peer.device_id);
                                            let remaining = peers_guard.values().filter(|p| p.authenticated).count();
                                            drop(peers_guard);
                                            if let Some(handler) = on_peer_disconnect_reader.lock().await.as_ref() {
                                                handler(peer.device_id, remaining);
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("{TAG} Accept error: {e}");
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the server.
    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        let mut peers = self.peers.lock().await;
        peers.clear();
        eprintln!("{TAG} Server stopped");
    }

    /// Broadcast a live change to all authenticated peers.
    pub async fn broadcast_live_change(&self, entity: SyncEntity, exclude_device_id: Option<&str>) {
        let change_id = generate_id();
        let msg = LanSyncMessage::LiveChange {
            change_id,
            entity: entity.clone(),
            origin_device_id: None,
        };
        let json = serialize_message(&msg);

        let mut peers = self.peers.lock().await;
        for peer in peers.values_mut() {
            if !peer.authenticated {
                continue;
            }
            if let Some(exclude) = exclude_device_id {
                if peer.device_id == exclude {
                    continue;
                }
            }
            if !peer.sync_complete {
                peer.queued_live_changes.push(entity.clone());
                continue;
            }
            let _ = peer.tx.send(Message::Text(json.clone()));
        }
    }

    /// Update the version vector for a locally-mutated entity.
    pub async fn update_entity_hlc(&self, entity_id: &str) -> String {
        let device_id = self.device_id.read().await.clone();
        let hlc_str = HLC::now(&device_id).to_string();
        let mut vector = load_version_vector(&self.storage).await;
        vector.insert(entity_id.to_string(), hlc_str.clone());
        save_version_vector(&self.storage, &vector).await;
        hlc_str
    }

    async fn load_known_peers(&self) {
        if let Some(raw) = self.storage.get_kv(KNOWN_PEERS_KEY).await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
                if parsed.is_array() {
                    if let Ok(peers) = serde_json::from_str::<Vec<PeerRecord>>(&raw) {
                        let removed = load_removed_peer_ids(&self.storage).await;
                        *self.known_peer_records.lock().await = peers
                            .into_iter()
                            .filter(|peer| !removed.iter().any(|id| id == &peer.device_id))
                            .collect();
                    }
                }
            }
        }
    }

    /// Purge stale self-referencing peer records. A record is "self" if:
    ///   - its device_id matches our current device_id, OR
    ///   - every address in its list is one of our own addresses (prior-run
    ///     phantom with a different device_id).
    ///
    /// Called on start after loading known peers. Prevents self-connect loops
    /// where stale records from dev iterations fire SyncClients that connect
    /// to our own SyncServer.
    pub async fn purge_self_peers(&self) -> usize {
        let my_device_id = self.device_id.read().await.clone();
        let my_addresses = self.own_addresses.read().await.clone();
        let mut known = self.known_peer_records.lock().await;
        let before = known.len();
        known.retain(|p| {
            if p.device_id == my_device_id {
                return false;
            }
            if p.addresses.is_empty() {
                return true;
            }
            let all_ours = p.addresses.iter().all(|a| my_addresses.contains(a));
            !all_ours
        });
        let removed = before - known.len();
        if removed > 0 {
            eprintln!("{TAG} Purged {removed} stale self-peer records");
            save_known_peers(&self.storage, &known).await;
        }
        removed
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------
