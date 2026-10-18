use super::super::current_ready_peer;
use super::server_session_cleanup::finish_rejected_session;
use super::server_session_host::ServerSessionHostAdapter;
use super::{signed_integration_write_is_authorized, *};
use ark_sync::outbound_snapshot::OutboundSnapshotProgress;
use ark_sync::server_session::run_connected_server_session_controlled;
use ark_sync::server_session::{ConnectedServerSession, ConnectedServerSessionHost};
use tokio::sync::oneshot;

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
        if ark_sync::protocol::requires_registry_admission(message) && ready_peer.is_none() {
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
                ready_peer
                    .as_ref()
                    .is_some_and(|ready| ready.matches(self.peer_id, peer))
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
        let transport_tasks = self.transport_tasks.clone();
        let session_host: Arc<dyn ConnectedServerSessionHost> =
            Arc::new(ServerSessionHostAdapter {
                peers: peers.clone(),
                known_peer_records: known_peer_records.clone(),
                storage: storage.clone(),
                device_id: device_id.clone(),
                own_addresses: own_addresses.clone(),
                on_peer_connect: on_peer_connect.clone(),
                on_registry_events: on_registry_events.clone(),
            });
        let (accept_ready_tx, accept_ready_rx) = oneshot::channel();

        // Ping ticker shares the accept task: a separate ping task had no
        // shutdown path and leaked (plus duplicated pings) on every restart.
        let accept_task = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(PING_INTERVAL_MS));
            ticker.tick().await;
            let _ = accept_ready_tx.send(());
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let peer_ids = {
                            let peers_guard = peers.lock().await;
                            for peer in peers_guard.values() {
                            if peer.is_ready() {
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
                                    peer.is_ready()
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
                                if let Some(transport_task) = session_io.take_transport_task() {
                                    track_task(&transport_tasks, transport_task);
                                }
                                let session_sender = session_io.sender();
                                let tx = PeerMessageSender::session(session_sender.clone());
                                let (session, mut session_actor, _lease, mut activation) =
                                    match ark_sync::server_session::ServerSessionHandle::channel(
                                        std::num::NonZeroUsize::new(64)
                                            .expect("session intent capacity is nonzero"),
                                    ) {
                                        Ok(session) => session,
                                        Err(error) => {
                                            eprintln!(
                                            "{TAG} session lease allocation failed: {error:?}"
                                            );
                                            let finish_result =
                                                finish_rejected_session(session_io, &session_sender)
                                                    .await;
                                            if let Err(finish_error) = finish_result {
                                                eprintln!(
                                                    "{TAG} rejected peer shutdown: {:?}",
                                                    finish_error
                                                );
                                            }
                                            continue;
                                        }
                                    };
                                let hello_request =
                                    ark_sync::server_session::ResponderHelloRequest {
                                        device_id: device_id.read().await.clone(),
                                        device_name: device_name.read().await.clone(),
                                        space_id: space_id.read().await.clone(),
                                        addresses: own_addresses.read().await.clone(),
                                        auth_secret: auth_secret.read().await.clone(),
                                        task_merge_bootstrap_v2,
                                        platform: Some(ark_net::host::local_platform()),
                                        app_version: ark_net::host::app_version(),
                                    };
                                session_actor.configure_connected(ConnectedServerSession::new(
                                    hello_request,
                                    storage.clone(),
                                    session_host.clone(),
                                ));

                                let peer_state = PeerState {
                                    session: session.clone(),
                                    device_id: String::new(),
                                    device_name: String::new(),
                                    addresses: Vec::new(),
                                    platform: None,
                                    app_version: None,
                                    outbound_snapshot: OutboundSnapshotProgress::not_sent(),
                                    negotiated_protocol: None,
                                    in_flight_feed: None,
                                    tx: tx.clone(),
                                };
                                let activated = {
                                    let mut peers = peers.lock().await;
                                    peers.insert(peer_id, peer_state);
                                    if let Err(error) = activation.activate() {
                                        peers.remove(&peer_id);
                                        eprintln!(
                                            "{TAG} could not activate peer session: {error:?}"
                                        );
                                        false
                                    } else {
                                        true
                                    }
                                };
                                if !activated {
                                    let finish_result =
                                        finish_rejected_session(session_io, &session_sender).await;
                                    if let Err(error) = finish_result {
                                        eprintln!(
                                            "{TAG} rejected peer shutdown: {:?}",
                                            error
                                        );
                                    }
                                    continue;
                                }

                                // Reader task
                                let ctx = SyncDataPorts {
                                    peers: peers.clone(),
                                    storage: storage.clone(),
                                    space_id: space_id.clone(),
                                    device_id: device_id.clone(),
                                    own_addresses: own_addresses.clone(),
                                    known_peer_records: known_peer_records.clone(),
                                    on_change: on_change.clone(),
                                    on_new_peer_discovered: on_new_peer_discovered.clone(),
                                };
                                let peers_reader = peers.clone();
                                let on_peer_disconnect_reader = on_peer_disconnect.clone();

                                track_task(&conn_tasks, tokio::spawn(async move {
                                    let result = run_connected_server_session_controlled(
                                        &mut session_io,
                                        &mut session_actor,
                                        &mut sync_server_messages::ConnectedSessionHandler {
                                            ctx: &ctx,
                                            peer_id,
                                        },
                                    )
                                    .await;
                                    if let ark_sync::server_session::DirectServerSessionEnd::Failed(
                                        error,
                                    ) = result
                                    {
                                        eprintln!("{TAG} peer session transport failed: {error}");
                                    }
                                    if let Err(error) = session_io.finish().await {
                                        eprintln!(
                                            "{TAG} peer transport shutdown failed: {error:?}"
                                        );
                                    }

                                    // Peer disconnected
                                    let mut peers_guard = peers_reader.lock().await;
                                    if let Some(peer) = peers_guard.remove(&peer_id) {
                                        if peer.is_ready() {
                                            eprintln!(
                                                "{TAG} Peer disconnected: {} ({})",
                                                peer.device_name, peer.device_id
                                            );
                                            let remaining = peers_guard
                                                .values()
                                                .filter(|p| p.is_ready())
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
        accept_ready_rx
            .await
            .map_err(|_| "Sync server stopped before accepting connections".to_owned())?;

        Ok(())
    }
}
