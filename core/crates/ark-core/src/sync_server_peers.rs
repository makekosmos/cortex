use super::*;

impl SyncServer {
    /// Stop the server.
    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        // Join the accept/ticker task: until it exits it holds a clone of
        // `storage`, keeping the test db connection open past teardown.
        if let Some(task) = self.accept_task.lock().await.take() {
            if let Err(error) = task.await {
                eprintln!("{TAG} accept task ended abnormally: {error}");
            }
        }
        let mut peers = self.peers.lock().await;
        peers.clear();
        eprintln!("{TAG} Server stopped");
    }

    /// Broadcast a live change to all authenticated peers.
    pub async fn broadcast_live_change(&self, entity: SyncEntity, exclude_device_id: Option<&str>) {
        let Some(entity) = self.storage.filter_outgoing_entity(&entity) else {
            return;
        };
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
            let _ = peer.tx.send(Message::Text(json.clone().into()));
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

    pub(super) async fn load_known_peers(&self) {
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
