use super::super::*;

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;

pub(super) async fn handle_registry_admission(
    ctx: &MessageContext,
    peer_id: usize,
    message: LanSyncMessage,
) {
    let (mut admission, tx, device_id) = {
        let mut peers = ctx.peers.lock().await;
        let Some(peer) = peers.get_mut(&peer_id) else {
            return;
        };
        let Some(admission) = peer.registry_admission.take() else {
            let _ = peer.tx.send(Message::Close(None));
            return;
        };
        (admission, peer.tx.clone(), peer.device_id.clone())
    };

    let update = match admission.handle(ctx.storage.as_ref(), message).await {
        Ok(update) => update,
        Err(error) => {
            eprintln!("{TAG} direct registry admission failed for {device_id}: {error}");
            reject_peer(ctx, peer_id, &tx).await;
            return;
        }
    };
    let registry_session_id = admission.session_id().map(str::to_owned);
    broadcast_registry_events(ctx, peer_id, &update.applied_events).await;
    let preserve_pending_peer = update.close_session.then_some(peer_id);
    if revoke_disabled_sessions(
        ctx,
        &update.disabled_device_ids,
        preserve_pending_peer,
        !update.local_device_is_admitted,
    )
    .await
    {
        return;
    }
    if !update.applied_events.is_empty() {
        if let Some(handler) = ctx.on_registry_events.lock().await.as_ref() {
            handler(update.applied_events.clone());
        }
    }
    for outgoing in update.outgoing {
        send_msg(&tx, &outgoing);
    }
    if update.close_session && update.exchange_complete {
        reject_peer(ctx, peer_id, &tx).await;
        return;
    }

    let (became_ready, peer_entry) = {
        let mut peers = ctx.peers.lock().await;
        let Some(peer) = peers.get_mut(&peer_id) else {
            return;
        };
        let became_ready = update.became_ready && peer.ready_peer.is_none();
        peer.registry_admission = Some(admission);
        let entry = PeerEntry {
            device_id: peer.device_id.clone(),
            device_name: peer.device_name.clone(),
            platform: peer.platform.clone(),
            app_version: peer.app_version.clone(),
        };
        (became_ready, entry)
    };
    if !became_ready {
        return;
    }

    let selective_policy_capability = ctx
        .peers
        .lock()
        .await
        .get(&peer_id)
        .and_then(|peer| peer.negotiated_protocol.as_ref())
        .is_some_and(|protocol| {
            protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::SelectiveSyncPolicyV1)
        });
    let policy_bound_delivery = ctx
        .peers
        .lock()
        .await
        .get(&peer_id)
        .and_then(|peer| peer.negotiated_protocol.as_ref())
        .is_some_and(|protocol| {
            protocol
                .capabilities
                .contains(&crate::protocol::ProtocolCapability::PolicyBoundDeliveryV1)
        });
    if !policy_bound_delivery {
        let generation = match ctx
            .storage
            .selective_sync_policy_generation_for_peer(&device_id)
            .await
        {
            Ok(generation) => generation,
            Err(error) => {
                eprintln!("{TAG} policy generation lookup failed for {device_id}: {error}");
                reject_peer(ctx, peer_id, &tx).await;
                return;
            }
        };
        if let Err(error) = ctx
            .storage
            .begin_outbound_policy(&device_id, generation)
            .await
        {
            eprintln!("{TAG} direct admission snapshot baseline failed: {error}");
            reject_peer(ctx, peer_id, &tx).await;
            return;
        }
    }

    let Some(registry_session_id) = registry_session_id else {
        reject_peer(ctx, peer_id, &tx).await;
        return;
    };
    let ready_peer = match ReadyPeer::new(peer_id, device_id.clone(), registry_session_id) {
        Some(ready_peer) => ready_peer,
        None => {
            reject_peer(ctx, peer_id, &tx).await;
            return;
        }
    };
    if !ctx
        .storage
        .sync_session_is_admitted(&device_id)
        .await
        .unwrap_or(false)
    {
        reject_peer(ctx, peer_id, &tx).await;
        return;
    }

    if selective_policy_capability {
        let local_device_id = ctx.device_id.read().await.clone();
        match ctx.storage.load_local_sync_profile(&local_device_id).await {
            Ok(Some(profile)) => send_msg(&tx, &crate::protocol::sync_policy_message(&profile)),
            Ok(None) => send_msg(&tx, &crate::protocol::default_sync_policy_message()),
            Err(error) => {
                eprintln!("{TAG} local sync policy load failed: {error}");
                reject_peer(ctx, peer_id, &tx).await;
                return;
            }
        }
    }

    let activated = {
        let mut peers = ctx.peers.lock().await;
        let stale_ids = peers
            .iter()
            .filter_map(|(candidate_id, peer)| {
                (*candidate_id != peer_id
                    && peer.ready_peer.is_some()
                    && peer.device_id == device_id)
                    .then_some(*candidate_id)
            })
            .collect::<Vec<_>>();
        for stale_id in stale_ids {
            if let Some(stale) = peers.get_mut(&stale_id) {
                eprintln!(
                    "{TAG} Evicting stale admitted session for {}",
                    stale.device_name
                );
                stale.ready_peer = None;
                let _ = stale.tx.send(Message::Close(None));
            }
            peers.remove(&stale_id);
        }
        peers.get_mut(&peer_id).is_some_and(|peer| {
            if peer.ready_peer.is_some() || peer.device_id != device_id {
                return false;
            }
            peer.ready_peer = Some(ready_peer);
            peer.sync_started = !policy_bound_delivery;
            true
        })
    };
    if !activated {
        reject_peer(ctx, peer_id, &tx).await;
        return;
    }

    let peer_addresses = {
        let peers = ctx.peers.lock().await;
        peers
            .get(&peer_id)
            .map(|peer| peer.addresses.clone())
            .unwrap_or_default()
    };
    let local_device_id = ctx.device_id.read().await.clone();
    let local_addresses = ctx.own_addresses.read().await.clone();
    let is_self = device_id == local_device_id
        || (!peer_addresses.is_empty()
            && peer_addresses
                .iter()
                .all(|address| local_addresses.contains(address)));
    if !is_self {
        let record = PeerRecord {
            device_id: peer_entry.device_id.clone(),
            device_name: peer_entry.device_name.clone(),
            addresses: peer_addresses,
            last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            last_address: None,
            platform: peer_entry.platform.clone(),
            app_version: peer_entry.app_version.clone(),
        };
        let mut known = ctx.known_peer_records.lock().await;
        *known = merge_peer_records(&known, &[record]);
        if let Err(error) = save_known_peers(&ctx.storage, &known).await {
            eprintln!("{TAG} failed to persist admitted peer metadata: {error}");
        }
    }

    if let Some(handler) = ctx.on_peer_connect.lock().await.as_ref() {
        handler(device_id.clone());
    }
    if !policy_bound_delivery {
        let vector = match load_version_vector(&ctx.storage).await {
            Ok(vector) => vector,
            Err(error) => {
                eprintln!("{TAG} failed to load sync version vector: {error}");
                reject_peer(ctx, peer_id, &tx).await;
                return;
            }
        };
        send_msg(
            &tx,
            &LanSyncMessage::VersionVector {
                vector,
                origin_device_id: None,
            },
        );
    }
    let known = ctx.known_peer_records.lock().await.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        send_msg(&tx, &LanSyncMessage::PeerList { peers: known });
    });
    eprintln!("{TAG} Direct registry admission ready for {device_id}");
}

pub(super) async fn start_sync_after_policy(
    ctx: &MessageContext,
    peer_id: usize,
    ready_peer: &ReadyPeer,
) -> Result<(), String> {
    let tx = {
        let mut peers = ctx.peers.lock().await;
        let Some(peer) = peers.get_mut(&peer_id) else {
            return Err("sync policy session disappeared".into());
        };
        if peer.ready_peer.as_ref() != Some(ready_peer) {
            return Err("sync policy session changed before sync start".into());
        }
        if peer.sync_started {
            return Ok(());
        }
        peer.sync_started = true;
        peer.tx.clone()
    };
    let identity = ctx
        .storage
        .accepted_policy_identity_for_peer(&ready_peer.device_id)
        .await?;
    let Some(identity) = identity else {
        return Err("accepted sync policy identity is missing".into());
    };
    ctx.storage
        .begin_outbound_policy_identity(&ready_peer.device_id, &identity)
        .await?;
    let vector = load_version_vector(&ctx.storage).await?;
    send_msg(
        &tx,
        &LanSyncMessage::VersionVector {
            vector,
            origin_device_id: None,
        },
    );
    Ok(())
}

async fn broadcast_registry_events(
    ctx: &MessageContext,
    source_peer_id: usize,
    events: &[crate::domain::device_registry::RegistryEvent],
) {
    if events.is_empty() {
        return;
    }
    let peers = ctx.peers.lock().await;
    for (peer_id, peer) in peers.iter() {
        if *peer_id == source_peer_id || peer.ready_peer.is_none() {
            continue;
        }
        let Some(session_id) = peer
            .registry_admission
            .as_ref()
            .and_then(|admission| admission.session_id())
        else {
            continue;
        };
        for event in events {
            send_msg(
                &peer.tx,
                &LanSyncMessage::RegistryEventUpdate {
                    session_id: session_id.to_owned(),
                    event: event.clone(),
                },
            );
        }
    }
}

async fn revoke_disabled_sessions(
    ctx: &MessageContext,
    disabled_device_ids: &[String],
    preserve_pending_peer: Option<usize>,
    local_device_revoked: bool,
) -> bool {
    let disabled = disabled_device_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let local_device_id = ctx.device_id.read().await.clone();
    let local_device_disabled = local_device_revoked || disabled.contains(local_device_id.as_str());
    if disabled_device_ids.is_empty() && !local_device_disabled {
        return false;
    }
    {
        let mut peers = ctx.peers.lock().await;
        let revoked_ids = peers
            .iter()
            .filter_map(|(peer_id, peer)| {
                let is_disabled =
                    local_device_disabled || disabled.contains(peer.device_id.as_str());
                let is_preserved_pending_peer = Some(*peer_id) == preserve_pending_peer
                    && peer.ready_peer.is_none()
                    && !local_device_disabled;
                (is_disabled && !is_preserved_pending_peer).then_some(*peer_id)
            })
            .collect::<Vec<_>>();
        for peer_id in &revoked_ids {
            if let Some(peer) = peers.get_mut(peer_id) {
                peer.ready_peer = None;
                peer.sync_complete = false;
                peer.in_flight_feed_position = None;
                peer.pending_initial_snapshot_ack = None;
                let _ = peer.tx.send(Message::Close(None));
            }
            peers.remove(peer_id);
        }
    }
    let mut known = ctx.known_peer_records.lock().await;
    known.retain(|peer| !local_device_disabled && !disabled.contains(peer.device_id.as_str()));
    if let Err(error) = save_known_peers(&ctx.storage, &known).await {
        eprintln!("{TAG} failed to persist registry revocation in peer list: {error}");
    }
    local_device_disabled
}

pub(super) async fn reject_peer(
    ctx: &MessageContext,
    peer_id: usize,
    tx: &mpsc::UnboundedSender<Message>,
) {
    if let Some(mut peer) = ctx.peers.lock().await.remove(&peer_id) {
        peer.ready_peer = None;
    }
    let _ = tx.send(Message::Close(None));
}
