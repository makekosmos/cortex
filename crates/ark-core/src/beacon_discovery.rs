impl BroadcastDiscovery {
    pub fn new() -> Self {
        Self {
            control_tx: Mutex::new(None),
            on_peer_discovered: Mutex::new(None),
            seen_peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub async fn set_on_peer_discovered(&self, handler: OnPeerDiscoveredCallback) {
        *self.on_peer_discovered.lock().await = Some(handler);
    }
    #[doc(hidden)]
    pub async fn process_inbound(
        &self,
        payload: &BeaconPayload,
        sender_ip: &str,
        own_space_id: &str,
        own_device_id: &str,
    ) {
        self.process_inbound_inner(payload, sender_ip, own_space_id, own_device_id, now_ms())
            .await;
    }
    async fn process_inbound_inner(
        &self,
        payload: &BeaconPayload,
        sender_ip: &str,
        own_space_id: &str,
        own_device_id: &str,
        now_val: u64,
    ) {
        if payload.t != BEACON_TYPE {
            return;
        }
        if payload.s != own_space_id {
            return;
        }
        if payload.d.is_empty() {
            return;
        }
        if payload.d == own_device_id {
            return; // self-reject
        }
        let port = if payload.p == 0 {
            LAN_SYNC_PORT
        } else {
            payload.p
        };
        let sender_addr = format!("{sender_ip}:{port}");
        let mut merged: Vec<String> = Vec::new();
        if is_address_routable(&sender_addr) {
            merged.push(sender_addr.clone());
        }
        for addr in &payload.a {
            if !is_address_routable(addr) {
                continue;
            }
            if !merged.contains(addr) {
                merged.push(addr.clone());
            }
        }
        if merged.is_empty() {
            return;
        }
        let mut sorted = merged.clone();
        sorted.sort();
        let should_fire = {
            let mut seen = self.seen_peers.lock().await;
            let existing = seen.get(&payload.d).cloned();
            let (should_fire, new_state) =
                classify_beacon(existing.as_ref(), &payload.n, &sorted, now_val);
            seen.insert(payload.d.clone(), new_state);
            should_fire
        };
        if !should_fire {
            return;
        }
        let peer = BeaconPeer {
            device_id: payload.d.clone(),
            device_name: payload.n.clone(),
            address: sender_addr,
            addresses: merged,
        };
        let handler = self.on_peer_discovered.lock().await.clone();
        if let Some(handler) = handler.as_ref() {
            handler(peer);
        }
    }
    pub async fn sweep_stale(&self) -> usize {
        let cutoff = now_ms().saturating_sub(PEER_TTL_MS);
        self.sweep_stale_at(cutoff).await
    }
    async fn sweep_stale_at(&self, cutoff_ms: u64) -> usize {
        let mut seen = self.seen_peers.lock().await;
        let before = seen.len();
        seen.retain(|_, peer| peer.last_seen_ms >= cutoff_ms);
        before - seen.len()
    }
    pub async fn is_running(&self) -> bool {
        self.control_tx.lock().await.is_some()
    }
    pub async fn seen_peer_count(&self) -> usize {
        self.seen_peers.lock().await.len()
    }
    pub async fn start(self: &Arc<Self>, options: BroadcastDiscoveryOptions) -> Result<(), String> {
        self.stop().await;
        self.seen_peers.lock().await.clear();
        let socket = UdpSocket::bind(SocketAddr::new(
            std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            BEACON_PORT,
        ))
        .await
        .map_err(|e| format!("Failed to bind UDP {BEACON_PORT}: {e}"))?;
        socket.set_broadcast(true).ok();
        let socket = Arc::new(socket);
        let (control_tx, mut control_rx) = mpsc::unbounded_channel::<Control>();
        *self.control_tx.lock().await = Some(control_tx);
        let options_arc = Arc::new(Mutex::new(options));
        let self_clone = self.clone();
        let options_for_loops = options_arc.clone();
        let socket_for_recv = socket.clone();
        let socket_for_send = socket.clone();
        let self_for_recv = self_clone.clone();
        let self_for_sweep = self_clone.clone();
        let recv_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            loop {
                match socket_for_recv.recv_from(&mut buf).await {
                    Ok((len, peer)) => {
                        let text = match std::str::from_utf8(&buf[..len]) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let payload: BeaconPayload = match serde_json::from_str(text) {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let opts = options_for_loops.lock().await.clone();
                        let sender_ip = match peer.ip() {
                            std::net::IpAddr::V4(v4) => v4.to_string(),
                            std::net::IpAddr::V6(v6) => {
                                format!("[{}]", strip_ipv6_zone(&v6.to_string()))
                            }
                        };
                        self_for_recv
                            .process_inbound_inner(
                                &payload,
                                &sender_ip,
                                &opts.space_id,
                                &opts.device_id,
                                now_ms(),
                            )
                            .await;
                    }
                    Err(e) => {
                        eprintln!("{TAG} recv error: {e}");
                        break;
                    }
                }
            }
        });
        let options_for_send = options_arc.clone();
        let send_handle = tokio::spawn(async move {
            if let Err(e) = send_beacon_once(&socket_for_send, &options_for_send).await {
                eprintln!("{TAG} initial send error: {e}");
            }
            let mut ticker = interval(Duration::from_millis(BEACON_INTERVAL_MS));
            ticker.tick().await; // skip immediate first tick
            loop {
                ticker.tick().await;
                if let Err(e) = send_beacon_once(&socket_for_send, &options_for_send).await {
                    eprintln!("{TAG} send error: {e}");
                }
            }
        });
        let sweep_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(PEER_TTL_MS));
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let _ = self_for_sweep.sweep_stale().await;
            }
        });
        let options_for_control = options_arc.clone();
        tokio::spawn(async move {
            while let Some(msg) = control_rx.recv().await {
                match msg {
                    Control::Stop => {
                        recv_handle.abort();
                        send_handle.abort();
                        sweep_handle.abort();
                        break;
                    }
                    Control::UpdateOptions {
                        space_id,
                        device_id,
                        device_name,
                        ws_port,
                    } => {
                        let mut opts = options_for_control.lock().await;
                        if let Some(s) = space_id {
                            opts.space_id = s;
                        }
                        if let Some(d) = device_id {
                            opts.device_id = d;
                        }
                        if let Some(n) = device_name {
                            opts.device_name = n;
                        }
                        if let Some(p) = ws_port {
                            opts.ws_port = p;
                        }
                    }
                }
            }
        });
        Ok(())
    }
    pub async fn stop(&self) {
        if let Some(tx) = self.control_tx.lock().await.take() {
            let _ = tx.send(Control::Stop);
        }
        self.seen_peers.lock().await.clear();
    }
    pub async fn update_options(
        &self,
        space_id: Option<String>,
        device_id: Option<String>,
        device_name: Option<String>,
        ws_port: Option<u16>,
    ) {
        if let Some(tx) = self.control_tx.lock().await.as_ref() {
            let _ = tx.send(Control::UpdateOptions {
                space_id,
                device_id,
                device_name,
                ws_port,
            });
        }
        self.seen_peers.lock().await.clear();
    }
}
