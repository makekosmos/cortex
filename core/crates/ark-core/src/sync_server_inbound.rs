use super::*;

// Inbound replication handlers moved out of `handle_message`'s match — the
// arm bodies are unchanged, they just run as functions taking the shared
// SyncDataPorts.

pub(super) async fn handle_signed_integration_frame(
    ctx: &SyncDataPorts,
    peer_id: usize,
    frame: SignedSyncEnvelope,
) {
    let SyncDataPorts {
        peers,
        storage,
        space_id,
        device_id,
        ..
    } = ctx;
    let (tx, authenticated_peer_id) = {
        let peers_guard = peers.lock().await;
        match peers_guard.get(&peer_id) {
            Some(peer) if peer.is_ready() => (peer.tx.clone(), peer.device_id.clone()),
            _ => return,
        }
    };
    let expected_space_id = space_id.read().await.clone();
    let expected_recipient_node_id = device_id.read().await.clone();
    if frame.space_id != expected_space_id
        || frame.origin_node_id != authenticated_peer_id
        || frame.recipient_node_id != expected_recipient_node_id
    {
        return;
    }
    if storage
        .apply_signed_integration_frame(
            &frame,
            &expected_space_id,
            &authenticated_peer_id,
            &expected_recipient_node_id,
        )
        .await
        .is_ok()
    {
        send_msg(
            &tx,
            &LanSyncMessage::SignedIntegrationAck {
                message_id: frame.message_id,
                accepted: true,
            },
        )
        .await;
    }
}

pub(super) async fn handle_live_change(
    ctx: &SyncDataPorts,
    peer_id: usize,
    change_id: String,
    entity: SyncEntity,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) {
    let SyncDataPorts {
        peers,
        storage,
        on_change,
        ..
    } = ctx;
    let (tx, authenticated_peer_id, protocol) = {
        let peers_guard = peers.lock().await;
        match peers_guard.get(&peer_id) {
            Some(p) if p.is_ready() => (
                p.tx.clone(),
                p.device_id.clone(),
                p.negotiated_protocol.clone(),
            ),
            _ => return,
        }
    };
    if authenticated_peer_id.is_empty() {
        return;
    }

    let local_device_id = ctx.device_id.read().await.clone();
    let local_profile = match storage.load_local_sync_profile(&local_device_id).await {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!("{TAG} local policy identity read failed: {error}");
            return;
        }
    };
    let notification_entity = entity.clone();
    let prepared = match ark_sync::inbound_live_change::prepare_live_change(
        storage.as_ref(),
        ark_sync::inbound_live_change::IncomingLiveChange {
            protocol: protocol.as_ref(),
            local_profile: local_profile.as_ref(),
            local_device_id: &local_device_id,
            peer_device_id: &authenticated_peer_id,
            change_id: change_id.clone(),
            entity,
            policy_identity: policy_identity.clone(),
        },
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("{TAG} inbound live-change preparation failed: {error}");
            return;
        }
    };
    if prepared.receipt_recorded {
        send_msg(
            &tx,
            &LanSyncMessage::LiveReceiptAck {
                policy_identity: policy_identity.clone(),
                change_id: change_id.clone(),
            },
        )
        .await;
    }

    let result = match ark_sync::inbound_live_change::apply_live_change(
        storage.as_ref(),
        prepared,
        ark_sync::inbound_live_change::VersionVectorWriteMode::MergeConcurrent,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{TAG} Failed to apply live sync entity: {error}");
            return;
        }
    };
    match result.disposition {
        ark_sync::inbound_live_change::LiveChangeDisposition::Applied { notify_local } => {
            if notify_local {
                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(notification_entity);
                }
            }

            let peer_device_id = {
                let peers_guard = peers.lock().await;
                peers_guard
                    .get(&peer_id)
                    .map(|p| p.device_id.clone())
                    .unwrap_or_default()
            };
            broadcast_to_others(storage, peers, Some(&peer_device_id)).await;
            send_msg(
                &tx,
                &LanSyncMessage::LiveAck {
                    change_id,
                    policy_identity: policy_identity.clone(),
                },
            )
            .await;
        }
        ark_sync::inbound_live_change::LiveChangeDisposition::Stale => {
            send_msg(
                &tx,
                &LanSyncMessage::LiveAck {
                    change_id,
                    policy_identity: policy_identity.clone(),
                },
            )
            .await;
        }
        ark_sync::inbound_live_change::LiveChangeDisposition::Pending => {}
    }
}

pub(super) async fn handle_peer_list(
    ctx: &SyncDataPorts,
    peer_id: usize,
    incoming_peers: Vec<PeerRecord>,
) {
    let SyncDataPorts {
        peers,
        storage,
        device_id,
        own_addresses,
        known_peer_records,
        on_new_peer_discovered,
        ..
    } = ctx;
    let authenticated = {
        let peers_guard = peers.lock().await;
        peers_guard
            .get(&peer_id)
            .map(|p| p.is_ready())
            .unwrap_or(false)
    };
    if !authenticated {
        return;
    }

    let my_device_id = device_id.read().await.clone();
    let my_addresses = own_addresses.read().await.clone();
    // Blocked peers must not re-enter through gossiped peer lists —
    // otherwise a removed device resurrects in known_peers and gets
    // re-gossiped to the rest of the mesh.
    let removed = match load_removed_peer_ids(storage).await {
        Ok(removed) => removed,
        Err(error) => {
            eprintln!("{TAG} failed to load blocked peer list: {error}");
            return;
        }
    };
    // Filter self-references: our own device_id, or any record whose
    // addresses are all ours (stale phantom from prior runs).
    let filtered: Vec<PeerRecord> = incoming_peers
        .into_iter()
        .filter(|p| {
            if p.device_id == my_device_id {
                return false;
            }
            if removed.iter().any(|id| id == &p.device_id) {
                return false;
            }
            if !p.addresses.is_empty() && p.addresses.iter().all(|a| my_addresses.contains(a)) {
                return false;
            }
            true
        })
        .collect();

    let mut known = known_peer_records.lock().await;
    let before_count = known.len();
    *known = merge_peer_records(&known, &filtered);
    if let Err(error) = save_known_peers(storage, &known).await {
        eprintln!("{TAG} failed to persist known peers: {error}");
        return;
    }

    // Notify about new peers
    if let Some(handler) = on_new_peer_discovered.lock().await.as_ref() {
        for peer_rec in &*known {
            if peer_rec.device_id != my_device_id {
                handler(peer_rec.clone());
            }
        }
    }

    if known.len() > before_count {
        eprintln!(
            "{TAG} Peer list updated: {before_count} -> {} known peers",
            known.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_server::sync_server_tests::tests::MemBackend;

    #[tokio::test]
    async fn failed_live_entity_apply_does_not_ack() {
        let backend = Arc::new(MemBackend::new());
        backend.fail_authenticated_apply("injected write failure");
        let storage = backend as Arc<dyn SyncRuntimeStorage>;
        let server = SyncServer::new(storage.clone());
        let (tx, mut rx) = mpsc::unbounded_channel();
        server.peers.lock().await.insert(
            7,
            PeerState {
                session: test_server_session_handle("peer"),
                device_id: "peer".into(),
                device_name: "Peer".into(),
                addresses: Vec::new(),
                platform: None,
                app_version: None,
                outbound_snapshot:
                    ark_sync::outbound_snapshot::OutboundSnapshotProgress::acknowledged(),
                negotiated_protocol: None,
                in_flight_feed: None,
                tx: PeerMessageSender::test(tx),
            },
        );
        let ctx = SyncDataPorts {
            peers: server.peers.clone(),
            storage,
            space_id: server.space_id.clone(),
            device_id: server.device_id.clone(),
            own_addresses: server.own_addresses.clone(),
            known_peer_records: server.known_peer_records.clone(),
            on_change: server.on_change.clone(),
            on_new_peer_discovered: server.on_new_peer_discovered.clone(),
        };

        sync_server_inbound::handle_live_change(
            &ctx,
            7,
            "change-1".into(),
            SyncEntity {
                entity_type: "object".into(),
                id: "object-1".into(),
                data: serde_json::json!({"name": "object"})
                    .as_object()
                    .unwrap()
                    .clone(),
                hlc: "2026-10-16T00:00:00.000Z:000001:peer".into(),
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            },
            None,
        )
        .await;

        assert!(
            rx.try_recv().is_err(),
            "failed entity apply must not emit LiveAck"
        );
    }
}
