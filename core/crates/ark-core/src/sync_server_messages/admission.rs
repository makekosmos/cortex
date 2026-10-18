use super::super::*;

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;

pub(super) async fn handle_registry_admission(
    ctx: &MessageContext,
    peer_id: usize,
    session: &mut ark_sync::server_session::DirectServerSession,
    message: LanSyncMessage,
) {
    let (admission, tx, device_id) = {
        let mut peers = ctx.peers.lock().await;
        let Some(peer) = peers.get_mut(&peer_id) else {
            return;
        };
        let Some(admission) = peer.registry_admission.take() else {
            peer.session.close_after_flush();
            peer.tx.close_after_flush();
            return;
        };
        (admission, peer.tx.clone(), peer.device_id.clone())
    };
    let mut admission = Some(admission);

    let Some(admission_session) = admission.as_mut() else {
        return;
    };
    let progress = match admission_session
        .handle(ctx.storage.as_ref(), message)
        .await
    {
        Ok(progress) => progress,
        Err(error) => {
            eprintln!("{TAG} direct registry admission failed for {device_id}: {error}");
            reject_peer(ctx, peer_id, &tx).await;
            return;
        }
    };
    broadcast_registry_events(ctx, peer_id, &progress.applied_events).await;
    let preserve_pending_peer = progress.close_session.then_some(peer_id);
    let session_revoked = revoke_disabled_sessions(
        ctx,
        &progress.disabled_device_ids,
        preserve_pending_peer,
        !progress.local_device_is_admitted,
    )
    .await;
    if !progress.applied_events.is_empty() {
        if let Some(handler) = ctx.on_registry_events.lock().await.as_ref() {
            handler(progress.applied_events.clone());
        }
    }
    if session_revoked {
        return;
    }
    for outgoing in progress.outgoing {
        send_msg(&tx, &outgoing).await;
    }
    let ready_admission = match progress.status {
        ark_sync::server_session::AdmissionStatus::Ready(ready) => Some(ready),
        ark_sync::server_session::AdmissionStatus::Pending => None,
        ark_sync::server_session::AdmissionStatus::CloseSession => {
            reject_peer(ctx, peer_id, &tx).await;
            return;
        }
    };

    let (became_ready, peer_entry) = {
        let mut peers = ctx.peers.lock().await;
        let Some(peer) = peers.get_mut(&peer_id) else {
            return;
        };
        let became_ready = ready_admission.is_some() && peer.ready_peer.is_none();
        if !became_ready {
            peer.registry_admission = admission.take();
        }
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

    let Some(ready) = ready_admission.as_ref() else {
        return;
    };
    let policy_bound_delivery = ready.policy_bound_delivery();
    let Some(admission_ref) = admission.as_ref() else {
        reject_peer(ctx, peer_id, &tx).await;
        return;
    };
    if let Err(error) = admission_ref
        .begin_legacy_baseline(ctx.storage.as_ref())
        .await
    {
        eprintln!("{TAG} direct admission snapshot baseline failed: {error}");
        reject_peer(ctx, peer_id, &tx).await;
        return;
    }

    let registry_session_id = ready.registry_session_id().to_owned();
    let ready_peer = match ReadyPeer::new(
        peer_id,
        ready.peer_device_id().to_owned(),
        registry_session_id,
    ) {
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

    let local_device_id = ctx.device_id.read().await.clone();
    let local_policy = match admission_ref
        .local_sync_policy_message(ctx.storage.as_ref(), &local_device_id)
        .await
    {
        Ok(policy) => policy,
        Err(error) => {
            eprintln!("{TAG} local sync policy load failed: {error}");
            reject_peer(ctx, peer_id, &tx).await;
            return;
        }
    };

    let sync_start_policy = if policy_bound_delivery {
        ark_sync::server_session::DirectServerSyncStartPolicy::AfterPolicyPersistence
    } else {
        ark_sync::server_session::DirectServerSyncStartPolicy::Immediate
    };
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
                stale.session.close_after_flush();
                stale.tx.close_after_flush();
            }
            peers.remove(&stale_id);
        }
        match peers.get_mut(&peer_id) {
            Some(peer) if peer.ready_peer.is_none() && peer.device_id == device_id => {
                if !session.mark_ready(sync_start_policy) {
                    eprintln!("{TAG} peer handshake was not awaiting registry admission");
                    false
                } else {
                    peer.registry_admission = admission.take();
                    peer.ready_peer = Some(ready_peer);
                    true
                }
            }
            _ => false,
        }
    };
    if !activated {
        reject_peer(ctx, peer_id, &tx).await;
        return;
    }
    if let Some(policy) = local_policy {
        send_msg(&tx, &policy).await;
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
        let vector = match ctx.storage.load_version_vector().await {
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
        )
        .await;
    }
    let known = ctx.known_peer_records.lock().await.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        send_msg(&tx, &LanSyncMessage::PeerList { peers: known }).await;
    });
    eprintln!("{TAG} Direct registry admission ready for {device_id}");
}

pub(super) async fn start_sync_after_policy(
    ctx: &MessageContext,
    peer_id: usize,
    session: &mut ark_sync::server_session::DirectServerSession,
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
        peer.tx.clone()
    };
    if !session.start_sync_after_policy_persisted() {
        return Ok(());
    }
    let vector = ark_sync::server_session::begin_policy_bound_sync(
        ctx.storage.as_ref(),
        &ready_peer.device_id,
    )
    .await?;
    send_msg(
        &tx,
        &LanSyncMessage::VersionVector {
            vector,
            origin_device_id: None,
        },
    )
    .await;
    Ok(())
}

async fn broadcast_registry_events(
    ctx: &MessageContext,
    source_peer_id: usize,
    events: &[ark_domain::device_registry::RegistryEvent],
) {
    if events.is_empty() {
        return;
    }
    let targets = {
        let peers = ctx.peers.lock().await;
        peers
            .iter()
            .filter_map(|(peer_id, peer)| {
                if *peer_id == source_peer_id || peer.ready_peer.is_none() {
                    return None;
                }
                let session_id = peer
                    .registry_admission
                    .as_ref()
                    .and_then(|admission| admission.session_id())?;
                Some((peer.tx.clone(), session_id.to_owned()))
            })
            .collect::<Vec<_>>()
    };
    for (tx, session_id) in targets {
        for event in events {
            send_msg(
                &tx,
                &LanSyncMessage::RegistryEventUpdate {
                    session_id: session_id.clone(),
                    event: event.clone(),
                },
            )
            .await;
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
                peer.outbound_snapshot.reset();
                peer.in_flight_feed = None;
                peer.session.close_after_flush();
                peer.tx.close_after_flush();
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

pub(super) async fn reject_peer(ctx: &MessageContext, peer_id: usize, tx: &PeerMessageSender) {
    if let Some(mut peer) = ctx.peers.lock().await.remove(&peer_id) {
        peer.ready_peer = None;
        peer.session.close_after_flush();
    }
    tx.close_after_flush();
}
