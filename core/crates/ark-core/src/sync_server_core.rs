use super::*;

#[path = "sync_server_core/start.rs"]
mod start;
use crate::task_set::track_task;

async fn signed_integration_write_is_authorized(
    storage: &Arc<dyn SyncRuntimeStorage>,
    frame: &SignedSyncEnvelope,
    expected_space_id: &str,
    expected_origin_node_id: &str,
) -> bool {
    storage
        .validate_outbound_signed_integration_frame(
            frame,
            expected_space_id,
            expected_origin_node_id,
        )
        .await
        .is_ok()
}

impl SyncServer {
    pub async fn broadcast_registry_event(
        &self,
        event: crate::domain::device_registry::RegistryEvent,
    ) -> usize {
        let target = event.target_device_id().as_str();
        let revokes_target = matches!(
            event.operation(),
            crate::domain::device_registry::RegistryOperation::Disable
        );
        let mut peers = self.peers.lock().await;
        let mut revoked = Vec::new();
        for (peer_id, peer) in peers.iter_mut() {
            if !peer.ready_peer.is_some() {
                continue;
            }
            let Some(session_id) = peer
                .registry_admission
                .as_ref()
                .and_then(|admission| admission.session_id())
            else {
                continue;
            };
            let update = LanSyncMessage::RegistryEventUpdate {
                session_id: session_id.to_owned(),
                event: event.clone(),
            };
            if peer
                .tx
                .send(Message::Text(serialize_message(&update).into()))
                .is_err()
            {
                continue;
            }
            if revokes_target && peer.device_id == target {
                peer.ready_peer = None;
                peer.sync_complete = false;
                peer.in_flight_feed_position = None;
                peer.pending_initial_snapshot_ack = None;
                let _ = peer.tx.send(Message::Close(None));
                revoked.push(*peer_id);
            }
        }
        for peer_id in &revoked {
            peers.remove(peer_id);
        }
        drop(peers);

        if !revoked.is_empty() {
            let mut known = self.known_peer_records.lock().await;
            known.retain(|peer| peer.device_id != target);
            if let Err(error) = save_known_peers(&self.storage, &known).await {
                eprintln!("{TAG} failed to persist registry-revoked peer list: {error}");
            }
        }
        revoked.len()
    }

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
            .find(|peer| peer.ready_peer.is_some() && peer.device_id == recipient_device_id)
        else {
            return Err("target LAN peer is not authenticated".into());
        };
        peer.tx
            .send(Message::Text(
                serialize_message(&LanSyncMessage::SignedIntegrationFrame { frame }).into(),
            ))
            .map_err(|_| "LAN peer connection is closed".into())
    }
    pub fn new(storage: Arc<dyn SyncRuntimeStorage>) -> Self {
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
            accept_task: Arc::new(Mutex::new(None)),
            conn_tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
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
        peers.values().filter(|p| p.ready_peer.is_some()).count()
    }

    pub async fn get_known_peers(&self) -> Result<Vec<PeerRecord>, String> {
        // `sync.peers` is written by the transport path (iroh pairing)
        // too — refresh the in-memory copy so a transport-paired peer that
        // goes offline still shows as an offline row instead of vanishing
        // until the next start (KOS-367).
        self.load_known_peers().await?;
        Ok(self.known_peer_records.lock().await.clone())
    }

    pub async fn get_removed_peer_ids(&self) -> Result<Vec<String>, String> {
        load_removed_peer_ids(&self.storage).await
    }

    pub async fn block_peer(&self, device_id: &str) -> Result<(), String> {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return Ok(());
        }
        let mut removed = load_removed_peer_ids(&self.storage).await?;
        if !removed.iter().any(|id| id == device_id) {
            removed.push(device_id.to_string());
            save_removed_peer_ids(&self.storage, &removed).await?;
        }
        let mut known = self.known_peer_records.lock().await;
        let before = known.len();
        known.retain(|peer| peer.device_id != device_id);
        if known.len() != before {
            save_known_peers(&self.storage, &known).await?;
        }
        Ok(())
    }

    /// Block a peer and explicitly tear down any live inbound sessions for it.
    /// Returns the number of active sessions that were closed.
    pub async fn disconnect_peer(&self, device_id: &str) -> Result<usize, String> {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return Ok(0);
        }
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
                if peer.ready_peer.is_some() {
                    peer.ready_peer = None;
                    let _ = peer.tx.send(Message::Close(None));
                    closed += 1;
                }
            }
        }
        for peer_id in peer_ids {
            peers.remove(&peer_id);
        }
        drop(peers);

        // Update local discovery state even if persistence fails. Blocking a
        // peer is fallible, but must never prevent closing its live sessions.
        self.known_peer_records
            .lock()
            .await
            .retain(|peer| peer.device_id != device_id);
        self.block_peer(device_id).await.map(|()| closed)
    }

    pub(super) async fn is_peer_blocked(&self, device_id: &str) -> Result<bool, String> {
        Ok(load_removed_peer_ids(&self.storage)
            .await?
            .iter()
            .any(|id| id == device_id))
    }

    /// Dedup connected peers by device_id. Matches the `getConnectedPeerEntries`
    /// semantics of the TS sync server. Order: LinkedHashMap insertion order.
    pub async fn get_connected_peer_entries(&self) -> Vec<PeerEntry> {
        let peers = self.peers.lock().await;
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut entries: Vec<PeerEntry> = Vec::new();
        for peer in peers.values() {
            if !peer.ready_peer.is_some() {
                continue;
            }
            if peer.device_id.is_empty() {
                continue;
            }
            match seen.get(&peer.device_id) {
                Some(&index) => {
                    let entry = &mut entries[index];
                    entry.device_name.clone_from(&peer.device_name);
                    if peer.platform.is_some() {
                        entry.platform.clone_from(&peer.platform);
                    }
                    if peer.app_version.is_some() {
                        entry.app_version.clone_from(&peer.app_version);
                    }
                }
                None => {
                    seen.insert(peer.device_id.clone(), entries.len());
                    entries.push(PeerEntry {
                        device_id: peer.device_id.clone(),
                        device_name: peer.device_name.clone(),
                        platform: peer.platform.clone(),
                        app_version: peer.app_version.clone(),
                    });
                }
            }
        }
        entries
    }

    /// True if any authenticated session matches the given `device_id`.
    pub async fn is_connected_to(&self, device_id: &str) -> bool {
        let peers = self.peers.lock().await;
        peers
            .values()
            .any(|p| p.ready_peer.is_some() && p.device_id == device_id)
    }

    /// Record an externally-connected peer (e.g. a `SyncClient` we just dialled
    /// out to) so `get_known_peers` reflects the merged set. Mirrors the
    /// TS `registerExternalPeer` helper. `last_seen`/`last_address` are
    /// overwritten here; `platform`/`app_version` pass through from the
    /// caller's record (the client's authenticated Hello, or `None` for a
    /// beacon-discovered peer that hasn't connected yet).
    pub async fn register_external_peer(&self, record: PeerRecord) -> Result<(), String> {
        let my_device_id = self.device_id.read().await.clone();
        let my_addresses = self.own_addresses.read().await.clone();

        if record.device_id == my_device_id {
            return Ok(());
        }
        if !record.addresses.is_empty() && record.addresses.iter().all(|a| my_addresses.contains(a))
        {
            return Ok(());
        }

        if self.is_peer_blocked(&record.device_id).await? {
            return Ok(());
        }
        let new_record = PeerRecord {
            last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            last_address: None,
            ..record
        };
        let mut known = self.known_peer_records.lock().await;
        *known = merge_peer_records(&known, &[new_record]);
        save_known_peers(&self.storage, &known).await?;
        Ok(())
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
}

#[cfg(test)]
include!("sync_server_core/writer_tests_module.rs");
