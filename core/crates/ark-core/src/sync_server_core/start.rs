use super::super::{current_ready_peer, peer_message_allowed};
use super::{signed_integration_write_is_authorized, *};
use ark_sync::outbound_snapshot::OutboundSnapshotProgress;
use ark_sync::session_io::SessionIoEvent;

struct ServerPeerOutboundGuard {
    storage: Arc<dyn SyncRuntimeStorage>,
    peers: Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    space_id: Arc<RwLock<String>>,
    device_id: Arc<RwLock<String>>,
}

#[async_trait::async_trait]
impl ark_sync::session_io::SessionOutboundGuard for ServerPeerOutboundGuard {
    async fn allow(&self, message: &LanSyncMessage) -> bool {
        let ready_peer = if ark_sync::protocol::requires_registry_admission(message) {
            current_ready_peer(self.storage.as_ref(), &self.peers, self.peer_id).await
        } else {
            None
        };
        if !peer_message_allowed(ready_peer.as_ref(), self.peer_id, message) {
            return false;
        }

        let LanSyncMessage::SignedIntegrationFrame { frame } = message else {
            return true;
        };
        let peer_is_current = self
            .peers
            .lock()
            .await
            .get(&self.peer_id)
            .is_some_and(|peer| {
                peer.ready_peer.as_ref() == ready_peer.as_ref()
                    && peer.device_id == frame.recipient_node_id
            });
        let expected_space_id = self.space_id.read().await.clone();
        let expected_device_id = self.device_id.read().await.clone();
        if !peer_is_current
            || !signed_integration_write_is_authorized(
                &self.storage,
                frame,
                &expected_space_id,
                &expected_device_id,
            )
            .await
        {
            return false;
        }
        current_ready_peer(self.storage.as_ref(), &self.peers, self.peer_id)
            .await
            .as_ref()
            == ready_peer.as_ref()
    }
}

impl ServerPeerOutboundGuard {
    fn new(
        storage: Arc<dyn SyncRuntimeStorage>,
        peers: Arc<Mutex<HashMap<usize, PeerState>>>,
        peer_id: usize,
        space_id: Arc<RwLock<String>>,
        device_id: Arc<RwLock<String>>,
    ) -> Self {
        Self {
            storage,
            peers,
            peer_id,
            space_id,
            device_id,
        }
    }
}

impl SyncServer {
    /// Start the WS server on an explicit bind address for embedders and tests.
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
        self.load_known_peers().await?;
        self.purge_self_peers().await?;

        let listener = ark_net::websocket::Listener::bind(bind_addr)
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
        let task_merge_bootstrap_v2 = self.task_merge_bootstrap_v2;
        let known_peer_records = self.known_peer_records.clone();
        let on_change = self.on_change.clone();
        let on_peer_connect = self.on_peer_connect.clone();
        let on_peer_disconnect = self.on_peer_disconnect.clone();
        let on_new_peer_discovered = self.on_new_peer_discovered.clone();
        let on_registry_events = self.on_registry_events.clone();
        let next_peer_id = self.next_peer_id.clone();
        let conn_tasks = self.conn_tasks.clone();

        // Ping ticker shares the accept task: a separate ping task had no
        // shutdown path and leaked (plus duplicated pings) on every restart.
        let accept_task = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(PING_INTERVAL_MS));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let peer_ids = {
                            let peers_guard = peers.lock().await;
                            for peer in peers_guard.values() {
                            if peer.ready_peer.is_some() {
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                let _ = peer.tx.try_send(LanSyncMessage::Ping { ts });
                            }
                            }
                            peers_guard
                                .iter()
                                .filter(|(_, peer)| {
                                    peer.ready_peer.is_some()
                                        && peer.outbound_snapshot.has_been_acknowledged()
                                })
                                .map(|(peer_id, _)| *peer_id)
                                .collect::<Vec<_>>()
                        };
                        for peer_id in peer_ids {
                            if let Err(error) = sync_server_peers::pump_committed_feed(
                                &storage,
                                &peers,
                                peer_id,
                            )
                            .await
                            {
                                eprintln!("{TAG} periodic committed feed drain failed: {error}");
                            }
                            if let Err(error) = sync_server_peers::pump_task_merge_batch(
                                &storage,
                                &peers,
                                peer_id,
                            )
                            .await
                            {
                                eprintln!("{TAG} periodic task merge drain failed: {error}");
                            }
                        }
                    }
                    result = listener.accept() => {
                        match result {
                            Ok((ws_stream, _)) => {
                                let ordering = std::sync::atomic::Ordering::Relaxed;
                                let peer_id = next_peer_id.fetch_add(1, ordering);

                                let outbound_guard = Arc::new(ServerPeerOutboundGuard::new(
                                    storage.clone(),
                                    peers.clone(),
                                    peer_id,
                                    space_id.clone(),
                                    device_id.clone(),
                                ));
                                let mut session_io =
                                    match ark_net::websocket::session_io_with_guard(
                                        ws_stream,
                                        Some(outbound_guard),
                                    ) {
                                    Ok(session_io) => session_io,
                                    Err(error) => {
                                        eprintln!(
                                            "{TAG} could not start peer transport: {error:?}"
                                        );
                                        continue;
                                    }
                                };
                                let tx = PeerMessageSender::session(session_io.sender());

                                let peer_state = PeerState {
                                    device_id: String::new(),
                                    device_name: String::new(),
                                    addresses: Vec::new(),
                                    platform: None,
                                    app_version: None,
                                    ready_peer: None,
                                    sync_started: false,
                                    outbound_snapshot: OutboundSnapshotProgress::not_sent(),
                                    negotiated_protocol: None,
                                    registry_admission: None,
                                    server_hello: None,
                                    in_flight_feed_position: None,
                                    in_flight_feed_policy_identity: None,
                                    tx: tx.clone(),
                                };
                                peers.lock().await.insert(peer_id, peer_state);

                                // Reader task
                                let ctx = MessageContext {
                                    peers: peers.clone(),
                                    storage: storage.clone(),
                                    space_id: space_id.clone(),
                                    device_id: device_id.clone(),
                                    device_name: device_name.clone(),
                                    own_addresses: own_addresses.clone(),
                                    auth_secret: auth_secret.clone(),
                                    task_merge_bootstrap_v2,
                                    known_peer_records: known_peer_records.clone(),
                                    on_change: on_change.clone(),
                                    on_peer_connect: on_peer_connect.clone(),
                                    on_new_peer_discovered: on_new_peer_discovered.clone(),
                                    on_registry_events: on_registry_events.clone(),
                                };
                                let peers_reader = peers.clone();
                                let on_peer_disconnect_reader = on_peer_disconnect.clone();

                                track_task(&conn_tasks, tokio::spawn(async move {
                                    while let Some(event) = session_io.receive().await {
                                        match event {
                                            SessionIoEvent::Message(message) => {
                                                sync_server_messages::handle_message(
                                                    &ctx, peer_id, *message,
                                                )
                                                .await;
                                            }
                                            SessionIoEvent::MalformedMessage => {
                                                eprintln!(
                                                    "{TAG} ignoring malformed peer sync message"
                                                );
                                            }
                                            SessionIoEvent::Closed | SessionIoEvent::Failed(_) => {
                                                break;
                                            }
                                        }
                                    }
                                    if let Err(error) = session_io.finish().await {
                                        eprintln!(
                                            "{TAG} peer transport shutdown failed: {error:?}"
                                        );
                                    }

                                    // Peer disconnected
                                    let mut peers_guard = peers_reader.lock().await;
                                    if let Some(peer) = peers_guard.remove(&peer_id) {
                                        if peer.ready_peer.is_some() {
                                            eprintln!(
                                                "{TAG} Peer disconnected: {} ({})",
                                                peer.device_name, peer.device_id
                                            );
                                            let remaining = peers_guard
                                                .values()
                                                .filter(|p| p.ready_peer.is_some())
                                                .count();
                                            drop(peers_guard);
                                            let disconnect_reader =
                                                on_peer_disconnect_reader.lock().await;
                                            if let Some(handler) = disconnect_reader.as_ref() {
                                                handler(peer.device_id, remaining);
                                            }
                                        }
                                    }
                                }));
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
        *self.accept_task.lock().await = Some(accept_task);

        Ok(())
    }
}
