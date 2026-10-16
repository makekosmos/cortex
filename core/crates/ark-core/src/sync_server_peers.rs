use super::*;

const FEED_CHANGE_PREFIX: &str = "ark-feed-v1:";

pub(super) async fn pump_committed_feed(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
) -> Result<(), String> {
    let (device_id, ready, in_flight, receipt_ack) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.ready_peer.is_some() && peer.sync_complete,
            peer.in_flight_feed_position,
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
            }),
        )
    };
    if !ready || in_flight.is_some() {
        return Ok(());
    }

    let generation = storage
        .selective_sync_policy_generation_for_peer(&device_id)
        .await?;
    let progress = storage
        .begin_outbound_policy(&device_id, generation)
        .await?;
    if !progress.initial_snapshot.is_complete() {
        return Ok(());
    }
    let snapshot = storage.committed_feed_snapshot().await?;
    let page = storage
        .committed_feed_delivery_page(
            if receipt_ack {
                progress.receipt_through
            } else {
                progress.applied_through
            },
            snapshot.high_watermark,
            crate::db::MAX_FEED_PAGE_SIZE,
        )
        .await?;
    let deliveries = match page {
        crate::application::sync_storage::CommittedFeedDeliveryPage::Deliveries(deliveries) => {
            deliveries
        }
        crate::application::sync_storage::CommittedFeedDeliveryPage::TooLarge {
            position,
            entity_id,
            serialized_bytes,
            max_bytes,
        } => {
            return Err(format!(
                "committed_feed_entity_too_large:{position}:{entity_id}:\
                 {serialized_bytes}:{max_bytes}"
            ));
        }
    };

    for delivery in deliveries {
        let position = delivery.entry.position;
        storage
            .record_outbound_scan(&device_id, generation, position)
            .await?;
        let Some(entity) = storage
            .filter_outgoing_entity_for_peer(&device_id, &delivery.entity)
            .await?
        else {
            storage
                .acknowledge_outbound(
                    &device_id,
                    generation,
                    position,
                    crate::application::sync_storage::OutboundAck::Applied,
                )
                .await?;
            continue;
        };
        let tx = {
            let mut peers = peers.lock().await;
            let Some(peer) = peers.get_mut(&peer_id) else {
                return Ok(());
            };
            if !peer.ready_peer.is_some()
                || !peer.sync_complete
                || peer.in_flight_feed_position.is_some()
            {
                return Ok(());
            }
            peer.in_flight_feed_position = Some(position);
            peer.tx.clone()
        };
        let message = LanSyncMessage::LiveChange {
            change_id: format!("{FEED_CHANGE_PREFIX}{position}"),
            entity,
            origin_device_id: None,
        };
        if tx
            .send(Message::Text(serialize_message(&message).into()))
            .is_err()
        {
            if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
                peer.in_flight_feed_position = None;
            }
        }
        return Ok(());
    }

    if progress.scanned_through < snapshot.high_watermark {
        storage
            .record_outbound_scan(&device_id, generation, snapshot.high_watermark)
            .await?;
    }
    Ok(())
}

pub(super) async fn handle_committed_feed_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    change_id: &str,
) -> Result<(), String> {
    let Some(position) = change_id
        .strip_prefix(FEED_CHANGE_PREFIX)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return Ok(());
    };
    let (device_id, receipt_ack, current_flight) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
            }),
            peer.in_flight_feed_position == Some(position),
        )
    };
    let generation = storage
        .selective_sync_policy_generation_for_peer(&device_id)
        .await?;
    let progress = storage
        .begin_outbound_policy(&device_id, generation)
        .await?;
    if (receipt_ack && position > progress.receipt_through) || (!receipt_ack && !current_flight) {
        return Ok(());
    }
    storage
        .acknowledge_outbound(
            &device_id,
            generation,
            position,
            crate::application::sync_storage::OutboundAck::Applied,
        )
        .await?;
    if current_flight {
        if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
            peer.in_flight_feed_position = None;
        }
    }
    if current_flight {
        pump_committed_feed(storage, peers, peer_id).await?;
    }
    Ok(())
}

pub(super) async fn handle_committed_feed_receipt_ack(
    storage: &Arc<dyn SyncRuntimeStorage>,
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    change_id: &str,
) -> Result<(), String> {
    let Some(position) = change_id
        .strip_prefix(FEED_CHANGE_PREFIX)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return Ok(());
    };
    let (device_id, receipt_ack, current_flight) = {
        let peers = peers.lock().await;
        let Some(peer) = peers.get(&peer_id) else {
            return Ok(());
        };
        (
            peer.device_id.clone(),
            peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                protocol
                    .capabilities
                    .contains(&crate::protocol::ProtocolCapability::DurableReceiptAckV1)
            }),
            peer.in_flight_feed_position == Some(position),
        )
    };
    if !receipt_ack {
        return Ok(());
    }
    let generation = storage
        .selective_sync_policy_generation_for_peer(&device_id)
        .await?;
    let progress = storage
        .begin_outbound_policy(&device_id, generation)
        .await?;
    if !current_flight && position > progress.receipt_through {
        return Ok(());
    }
    storage
        .acknowledge_outbound(
            &device_id,
            generation,
            position,
            crate::application::sync_storage::OutboundAck::DurableReceipt,
        )
        .await?;
    if current_flight {
        if let Some(peer) = peers.lock().await.get_mut(&peer_id) {
            peer.in_flight_feed_position = None;
        }
        pump_committed_feed(storage, peers, peer_id).await?;
    }
    Ok(())
}

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
        // The accept task has ended, so no new connection can be recorded.
        // Each connection's reader holds `storage` and may be partway through
        // a db write. Ask the peers to close and drop the writers' senders,
        // then give the connection tasks a bounded chance to finish on their
        // own: aborting a reader mid-write would leave its blocking db call
        // running past the join (KOS-270). Whatever is still alive after the
        // bound is aborted.
        {
            let mut peers = self.peers.lock().await;
            for peer in peers.values() {
                let _ = peer.tx.send(Message::Close(None));
            }
            peers.clear();
        }
        let tasks: Vec<tokio::task::JoinHandle<()>> = self
            .conn_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        let aborts: Vec<tokio::task::AbortHandle> =
            tasks.iter().map(|t| t.abort_handle()).collect();
        let joined = async {
            for task in tasks {
                let _ = task.await;
            }
        };
        if tokio::time::timeout(Duration::from_secs(2), joined)
            .await
            .is_err()
        {
            for abort in aborts {
                abort.abort();
            }
        }
        eprintln!("{TAG} Server stopped");
    }

    /// Broadcast a live change to all authenticated peers.
    pub async fn broadcast_live_change(&self, exclude_device_id: Option<&str>) {
        let peer_ids = {
            let peers = self.peers.lock().await;
            let mut ids = Vec::new();
            for (peer_id, peer) in peers.iter() {
                if !peer.ready_peer.is_some() {
                    continue;
                }
                if let Some(exclude) = exclude_device_id {
                    if peer.device_id == exclude {
                        continue;
                    }
                }
                if !peer.sync_complete {
                    continue;
                }
                ids.push(*peer_id);
            }
            ids
        };
        for peer_id in peer_ids {
            if let Err(error) = pump_committed_feed(&self.storage, &self.peers, peer_id).await {
                eprintln!("{TAG} committed feed wake failed: {error}");
            }
        }
    }

    /// Update the version vector for a locally-mutated entity.
    pub async fn update_entity_hlc(&self, entity_id: &str) -> Result<String, String> {
        let device_id = self.device_id.read().await.clone();
        let hlc_str = HLC::now(&device_id).to_string();
        let mut vector = load_version_vector(&self.storage).await?;
        vector.insert(entity_id.to_string(), hlc_str.clone());
        save_version_vector(&self.storage, &vector).await?;
        Ok(hlc_str)
    }

    pub(super) async fn load_known_peers(&self) -> Result<(), String> {
        if let Some(peers) = self.storage.load_known_peers().await? {
            let removed = load_removed_peer_ids(&self.storage).await?;
            *self.known_peer_records.lock().await = peers
                .into_iter()
                .filter(|peer| !removed.iter().any(|id| id == &peer.device_id))
                .collect();
        }
        Ok(())
    }

    /// Purge stale self-referencing peer records. A record is "self" if:
    ///   - its device_id matches our current device_id, OR
    ///   - every address in its list is one of our own addresses (prior-run
    ///     phantom with a different device_id).
    ///
    /// Called on start after loading known peers. Prevents self-connect loops
    /// where stale records from dev iterations fire SyncClients that connect
    /// to our own SyncServer.
    pub async fn purge_self_peers(&self) -> Result<usize, String> {
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
            save_known_peers(&self.storage, &known).await?;
        }
        Ok(removed)
    }
}
