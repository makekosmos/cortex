use super::super::*;
use ark_sync::server_session::{
    ConnectedServerSessionHost, ServerConnectionLease, ServerSessionIntent,
};

pub(super) struct ServerSessionHostAdapter {
    pub(super) peers: Arc<Mutex<HashMap<usize, PeerState>>>,
    pub(super) known_peer_records: Arc<Mutex<Vec<PeerRecord>>>,
    pub(super) storage: Arc<dyn SyncRuntimeStorage>,
    pub(super) device_id: Arc<RwLock<String>>,
    pub(super) own_addresses: Arc<RwLock<Vec<String>>>,
    pub(super) on_peer_connect: Arc<Mutex<Option<OnPeerConnectCallback>>>,
    pub(super) on_registry_events: Arc<Mutex<Option<OnRegistryEventsCallback>>>,
}

#[async_trait::async_trait]
impl ConnectedServerSessionHost for ServerSessionHostAdapter {
    async fn broadcast_registry_events(
        &self,
        source: ServerConnectionLease,
        events: Vec<ark_domain::device_registry::RegistryEvent>,
    ) {
        let mut peers = self.peers.lock().await;
        for peer in peers.values_mut() {
            if peer.session.lease() == source || !peer.is_ready() {
                continue;
            }
            for event in &events {
                if peer
                    .session
                    .try_send(ServerSessionIntent::BroadcastRegistryEvent(event.clone()))
                    .is_err()
                {
                    peer.session.revoke();
                    peer.tx.close();
                    break;
                }
            }
        }
    }

    async fn revoke_disabled_sessions(
        &self,
        disabled_device_ids: Vec<String>,
        preserve_pending: Option<ServerConnectionLease>,
        local_device_revoked: bool,
    ) -> bool {
        let disabled = disabled_device_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        let local_device_id = self.device_id.read().await.clone();
        let local_disabled = local_device_revoked || disabled.contains(local_device_id.as_str());
        if !local_disabled && disabled.is_empty() {
            return false;
        }
        let mut peers = self.peers.lock().await;
        let revoked = peers
            .iter()
            .filter_map(|(id, peer)| {
                let is_disabled = local_disabled || disabled.contains(peer.device_id.as_str());
                let keep_pending = preserve_pending == Some(peer.session.lease())
                    && !peer.is_ready()
                    && !local_disabled;
                (is_disabled && !keep_pending).then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in revoked {
            if let Some(peer) = peers.remove(&id) {
                peer.session.close_after_flush();
                peer.tx.close_after_flush();
            }
        }
        drop(peers);

        let mut known = self.known_peer_records.lock().await;
        known.retain(|peer| !local_disabled && !disabled.contains(peer.device_id.as_str()));
        let _ = self.storage.store_known_peers(&known).await;
        local_disabled
    }

    async fn evict_replaced_sessions(
        &self,
        device_id: &str,
        current: ServerConnectionLease,
    ) -> bool {
        let mut peers = self.peers.lock().await;
        let replaced = peers
            .iter()
            .filter_map(|(id, peer)| {
                (peer.session.lease() != current && peer.is_ready() && peer.device_id == device_id)
                    .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in replaced {
            if let Some(peer) = peers.remove(&id) {
                peer.session.close_after_flush();
                peer.tx.close_after_flush();
            }
        }
        peers.values().any(|peer| {
            peer.session.matches_lease(current)
                && peer.session.control() == ark_sync::server_session::ServerSessionControl::Active
        })
    }

    async fn prepare_peer_connected(
        &self,
        lease: ServerConnectionLease,
        projection: ark_sync::server_session::ServerPeerProjection,
        protocol: ark_sync::protocol::NegotiatedProtocol,
    ) {
        let Some(device_id) = projection.device_id.clone() else {
            return;
        };
        let Some(device_name) = projection.device_name.clone() else {
            return;
        };
        let mut peers = self.peers.lock().await;
        let Some((_, peer)) = peers
            .iter_mut()
            .find(|(_, peer)| peer.session.matches_lease(lease))
        else {
            return;
        };
        peer.device_id = device_id.clone();
        peer.device_name = device_name;
        peer.addresses = projection.addresses.clone();
        peer.platform = projection.platform.clone();
        peer.app_version = projection.app_version.clone();
        peer.negotiated_protocol = Some(protocol);
        drop(peers);

        let local_device_id = self.device_id.read().await.clone();
        let own_addresses = self.own_addresses.read().await.clone();
        if device_id != local_device_id
            && !projection.addresses.is_empty()
            && !projection
                .addresses
                .iter()
                .all(|address| own_addresses.contains(address))
        {
            let record = PeerRecord {
                device_id: device_id.clone(),
                device_name: projection.device_name.unwrap_or_default(),
                addresses: projection.addresses,
                last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                last_address: None,
                platform: projection.platform,
                app_version: projection.app_version,
            };
            let mut known = self.known_peer_records.lock().await;
            *known = merge_peer_records(&known, &[record]);
            if let Err(error) = save_known_peers(&self.storage, &known).await {
                eprintln!("{TAG} failed to persist admitted peer metadata: {error}");
            }
        }
    }

    async fn notify_peer_connected(&self, device_id: String) {
        if let Some(handler) = self.on_peer_connect.lock().await.as_ref() {
            handler(device_id);
        }
    }

    async fn registry_events_applied(
        &self,
        events: Vec<ark_domain::device_registry::RegistryEvent>,
    ) {
        let callbacks = self.on_registry_events.lock().await;
        if let Some(handler) = callbacks.as_ref() {
            handler(events);
        }
    }

    async fn peer_list(&self) -> Vec<PeerRecord> {
        self.known_peer_records.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_server::sync_server_tests::tests::MemBackend;

    #[tokio::test]
    async fn registry_disable_removes_and_closes_matching_ready_session() {
        let storage = Arc::new(MemBackend::new()) as Arc<dyn SyncRuntimeStorage>;
        let session = crate::sync_server::test_server_session_handle("disabled-device");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let peer = PeerState {
            session: session.clone(),
            device_id: "disabled-device".to_owned(),
            device_name: "Old laptop".to_owned(),
            addresses: Vec::new(),
            platform: None,
            app_version: None,
            outbound_snapshot: ark_sync::outbound_snapshot::OutboundSnapshotProgress::not_sent(),
            negotiated_protocol: None,
            in_flight_feed: None,
            tx: PeerMessageSender::test(tx),
        };
        let peers = Arc::new(Mutex::new(HashMap::from([(7, peer)])));
        let known_peer_records = Arc::new(Mutex::new(vec![PeerRecord {
            device_id: "disabled-device".to_owned(),
            device_name: "Old laptop".to_owned(),
            addresses: Vec::new(),
            last_seen: String::new(),
            last_address: None,
            platform: None,
            app_version: None,
        }]));
        let host = ServerSessionHostAdapter {
            peers: peers.clone(),
            known_peer_records: known_peer_records.clone(),
            storage,
            device_id: Arc::new(RwLock::new("local-device".to_owned())),
            own_addresses: Arc::new(RwLock::new(Vec::new())),
            on_peer_connect: Arc::new(Mutex::new(None)),
            on_registry_events: Arc::new(Mutex::new(None)),
        };

        assert!(
            !host
                .revoke_disabled_sessions(vec!["disabled-device".to_owned()], None, false)
                .await
        );
        assert!(peers.lock().await.is_empty());
        assert!(known_peer_records.lock().await.is_empty());
        assert_eq!(
            session.control(),
            ark_sync::server_session::ServerSessionControl::CloseAfterFlush
        );
        assert!(matches!(rx.recv().await, Some(Message::Close(_))));
    }
}
