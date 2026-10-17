use super::*;

pub(super) async fn handle_live_message(ctx: &MessageContext, peer_id: usize, msg: LanSyncMessage) {
    let MessageContext { peers, storage, .. } = ctx;
    match msg {
        LanSyncMessage::SyncAck {
            batch_id,
            policy_identity,
            ..
        } => {
            let pending = {
                let peers_guard = peers.lock().await;
                peers_guard.get(&peer_id).and_then(|peer| {
                    peer.pending_initial_snapshot
                        .as_ref()
                        .filter(|pending| pending.batch_id == batch_id)
                        .map(|pending| (peer.device_id.clone(), pending.clone()))
                })
            };
            if let Some((peer_device_id, pending)) = pending {
                let completion = ark_sync::outbound_snapshot::acknowledge_snapshot(
                    storage.as_ref(),
                    &peer_device_id,
                    &pending,
                    &batch_id,
                    policy_identity.as_ref(),
                )
                .await;
                match completion {
                    Ok(true) => {
                        let completed = peers.lock().await.get_mut(&peer_id).is_some_and(|peer| {
                            if peer.pending_initial_snapshot.as_ref() != Some(&pending) {
                                return false;
                            }
                            peer.pending_initial_snapshot = None;
                            peer.sync_complete = true;
                            true
                        });
                        if !completed {
                            return;
                        }
                        if let Err(error) =
                            sync_server_peers::pump_committed_feed(storage, peers, peer_id).await
                        {
                            eprintln!(
                                "{TAG} committed feed pump failed for {peer_device_id}: {error}"
                            );
                        }
                        if let Err(error) =
                            sync_server_peers::pump_task_merge_batch(storage, peers, peer_id).await
                        {
                            eprintln!("{TAG} task merge pump failed for {peer_device_id}: {error}");
                        }
                    }
                    Ok(false) => {}
                    Err(error) => eprintln!("{TAG} outbound snapshot completion failed: {error}"),
                }
            }
        }

        LanSyncMessage::SignedIntegrationFrame { frame } => {
            sync_server_inbound::handle_signed_integration_frame(ctx, peer_id, frame).await
        }

        LanSyncMessage::SignedIntegrationAck { .. } => {}

        LanSyncMessage::LiveChange {
            change_id,
            entity,
            policy_identity,
            ..
        } => {
            sync_server_inbound::handle_live_change(
                ctx,
                peer_id,
                change_id,
                entity,
                policy_identity,
            )
            .await
        }

        LanSyncMessage::LiveAck {
            change_id,
            policy_identity,
        } => {
            if let Err(error) = sync_server_peers::handle_committed_feed_ack(
                storage,
                peers,
                peer_id,
                &change_id,
                policy_identity,
            )
            .await
            {
                eprintln!("{TAG} committed feed ACK failed: {error}");
            }
        }

        LanSyncMessage::LiveReceiptAck {
            change_id,
            policy_identity,
        } => {
            if let Err(error) = sync_server_peers::handle_committed_feed_receipt_ack(
                storage,
                peers,
                peer_id,
                &change_id,
                policy_identity,
            )
            .await
            {
                eprintln!("{TAG} committed feed receipt ACK failed: {error}");
            }
        }

        LanSyncMessage::TaskMergeBatch { batch } => {
            let (tx, peer_device_id, supported) = {
                let peers = peers.lock().await;
                let Some(peer) = peers.get(&peer_id) else {
                    return;
                };
                (
                    peer.tx.clone(),
                    peer.device_id.clone(),
                    peer.sync_complete
                        && peer.negotiated_protocol.as_ref().is_some_and(|protocol| {
                            protocol.capabilities.contains(
                                &ark_sync::protocol::ProtocolCapability::TaskMergeBootstrapV2,
                            )
                        }),
                )
            };
            if !supported {
                return;
            }
            let local_device_id = ctx.device_id.read().await.clone();
            match storage
                .apply_task_merge_batch(&peer_device_id, &local_device_id, &batch.identity, &batch)
                .await
            {
                Ok(ack) => {
                    let message = LanSyncMessage::TaskMergeBatchAck {
                        batch_id: ack.batch_id,
                        through_position: ack.through_position,
                        policy_identity: batch.identity,
                    };
                    send_msg(&tx, &message);
                    let peer_ids = peers.lock().await.keys().copied().collect::<Vec<_>>();
                    for peer_id in peer_ids {
                        if let Err(error) =
                            sync_server_peers::pump_task_merge_batch(storage, peers, peer_id).await
                        {
                            eprintln!("{TAG} task merge fanout failed: {error}");
                        }
                    }
                }
                Err(error) => eprintln!("{TAG} task merge batch rejected: {error}"),
            }
        }

        LanSyncMessage::TaskMergeBatchAck {
            batch_id,
            through_position,
            policy_identity,
        } => {
            if let Err(error) = sync_server_peers::handle_task_merge_batch_ack(
                storage,
                peers,
                peer_id,
                &batch_id,
                through_position,
                &policy_identity,
            )
            .await
            {
                eprintln!("{TAG} task merge batch ACK failed: {error}");
            }
        }

        LanSyncMessage::PeerList {
            peers: incoming_peers,
        } => sync_server_inbound::handle_peer_list(ctx, peer_id, incoming_peers).await,

        LanSyncMessage::Ping { ts } => {
            let tx = {
                let peers_guard = peers.lock().await;
                peers_guard.get(&peer_id).map(|p| p.tx.clone())
            };
            if let Some(tx) = tx {
                send_msg(&tx, &LanSyncMessage::Pong { ts });
            }
        }

        LanSyncMessage::Pong { .. } => {}

        // Pairing consent lives on the addressed (iroh) transport; a
        // rejection over the WS path is meaningless — drop it.
        LanSyncMessage::PairingRejected { .. } => {}
        LanSyncMessage::PairingAccepted { .. } => {}

        _ => {}
    }
}
