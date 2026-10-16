use super::*;

pub(super) async fn handle_hello(ctx: &MessageContext, peer_id: usize, msg: LanSyncMessage) {
    let MessageContext {
        peers,
        storage,
        space_id,
        device_id,
        device_name,
        own_addresses,
        auth_secret,
        known_peer_records,
        on_peer_connect,
        ..
    } = ctx;
    if let LanSyncMessage::Hello {
        protocol_version,
        device_id: peer_device_id,
        device_name: peer_device_name,
        space_id: peer_space_id,
        addresses,
        auth_nonce,
        auth_hmac,
        platform: peer_platform,
        app_version: peer_app_version,
        supported_protocol_versions,
        supported_capabilities,
        negotiated_capabilities,
        negotiation_hmac: peer_negotiation_hmac,
    } = msg
    {
        let repeated = peers
            .lock()
            .await
            .get(&peer_id)
            .is_some_and(|peer| peer.authenticated);
        if repeated {
            eprintln!("{TAG} Rejecting repeated hello on an authenticated connection");
            reject_hello(peers, peer_id).await;
            return;
        }
        let my_device_id = device_id.read().await.clone();
        let my_device_name = device_name.read().await.clone();
        let my_space_id = space_id.read().await.clone();
        let my_addresses = own_addresses.read().await.clone();

        if peer_space_id != my_space_id {
            eprintln!("{TAG} Rejecting peer hello from a different space");
            reject_hello(peers, peer_id).await;
            return;
        }

        if let Some(secret) = auth_secret.read().await.clone() {
            let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
                (Some(nonce), Some(hmac)) => {
                    verify_hello_auth_hmac(&secret, &my_space_id, &peer_device_id, nonce, hmac)
                }
                _ => false,
            };
            if !valid {
                eprintln!("{TAG} Rejecting peer hello with invalid HMAC");
                reject_hello(peers, peer_id).await;
                return;
            }
        }

        let secret = auth_secret.read().await.clone();
        let local_offer = ProtocolOffer::current(secret.is_some());
        if negotiated_capabilities.is_some() {
            send_protocol_rejection(
                peers,
                peer_id,
                &my_device_id,
                &peer_device_id,
                ProtocolNegotiationError::InvalidCapabilityOffer,
                &[],
            )
            .await;
            return;
        }
        let peer_offer = match ProtocolOffer::new(
            protocol_version,
            supported_protocol_versions.as_deref(),
            supported_capabilities.as_deref(),
        ) {
            Ok(offer) => offer,
            Err(error) => {
                send_protocol_rejection(peers, peer_id, &my_device_id, &peer_device_id, error, &[])
                    .await;
                return;
            }
        };
        let peer_offer_authenticated = match (
            secret.as_deref(),
            auth_nonce.as_deref(),
            peer_negotiation_hmac.as_deref(),
        ) {
            (Some(secret), Some(nonce), Some(hmac)) => verify_negotiation_offer_hmac(
                secret,
                &my_space_id,
                &peer_device_id,
                nonce,
                &peer_offer,
                hmac,
            ),
            (_, _, Some(_)) => false,
            _ => false,
        };
        if peer_negotiation_hmac.is_some() && !peer_offer_authenticated {
            send_protocol_rejection(
                peers,
                peer_id,
                &my_device_id,
                &peer_device_id,
                ProtocolNegotiationError::InvalidSelectionProof,
                &peer_offer.version_ids(),
            )
            .await;
            return;
        }
        let negotiated =
            match negotiate_protocol(&local_offer, &peer_offer, peer_offer_authenticated) {
                Ok(negotiated) => negotiated,
                Err(error) => {
                    send_protocol_rejection(
                        peers,
                        peer_id,
                        &my_device_id,
                        &peer_device_id,
                        error,
                        &peer_offer.version_ids(),
                    )
                    .await;
                    return;
                }
            };

        // Reject self-connect: a client claiming our own device_id is us
        // (stale phantom peer in storage or address looping back to us).
        if !peer_device_id.is_empty() && peer_device_id == my_device_id {
            eprintln!("{TAG} Rejecting self-connect: client claims our device_id {peer_device_id}");
            reject_hello(peers, peer_id).await;
            return;
        }
        let removed = match load_removed_peer_ids(storage).await {
            Ok(removed) => removed,
            Err(error) => {
                eprintln!("{TAG} failed to load blocked peer list: {error}");
                reject_hello(peers, peer_id).await;
                return;
            }
        };
        if !peer_device_id.is_empty() && removed.iter().any(|id| id == &peer_device_id) {
            eprintln!("{TAG} Rejecting blocked peer: {peer_device_id}");
            reject_hello(peers, peer_id).await;
            return;
        }

        let generation = storage.selective_sync_policy_generation();
        if let Err(error) = storage
            .begin_outbound_policy(&peer_device_id, generation)
            .await
        {
            eprintln!("{TAG} outbound snapshot baseline failed: {error}");
            reject_hello(peers, peer_id).await;
            return;
        }

        // Evict any prior authenticated sessions from the same device — a
        // new hello means a fresh connection, and the old session is stale.
        // Mark stale sessions unauthenticated so their close handler
        // doesn't fire an extra on_peer_disconnected.
        {
            let mut peers_guard = peers.lock().await;
            let stale_ids: Vec<usize> = peers_guard
                .iter()
                .filter_map(|(id, p)| {
                    if *id != peer_id && p.authenticated && p.device_id == peer_device_id {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for stale_id in stale_ids {
                if let Some(stale) = peers_guard.get_mut(&stale_id) {
                    eprintln!(
                        "{TAG} Evicting stale session for {} (superseded by new hello)",
                        stale.device_name
                    );
                    stale.authenticated = false;
                    let _ = stale.tx.send(Message::Close(None));
                }
                peers_guard.remove(&stale_id);
            }
        }

        let tx = {
            let mut peers_guard = peers.lock().await;
            if let Some(peer) = peers_guard.get_mut(&peer_id) {
                peer.device_id = peer_device_id.clone();
                peer.device_name = peer_device_name.clone();
                peer.addresses = addresses.clone().unwrap_or_default();
                peer.platform = peer_platform.clone();
                peer.app_version = peer_app_version.clone();
                peer.negotiated_protocol = Some(negotiated.clone());
                peer.authenticated = true;
                peer.tx.clone()
            } else {
                return;
            }
        };

        eprintln!("{TAG} Peer authenticated: {peer_device_name}");

        // Update known peer record — skip if it's a self-reference
        // (device_id match or all-self address list).
        {
            let peer_addresses = addresses.clone().unwrap_or_default();
            let is_self = peer_device_id == my_device_id
                || (!peer_addresses.is_empty()
                    && peer_addresses.iter().all(|a| my_addresses.contains(a)));
            if !is_self {
                let new_record = PeerRecord {
                    device_id: peer_device_id.clone(),
                    device_name: peer_device_name,
                    addresses: peer_addresses,
                    last_seen: chrono::Utc::now()
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    last_address: None,
                    platform: peer_platform,
                    app_version: peer_app_version,
                };
                let removed = match load_removed_peer_ids(storage).await {
                    Ok(removed) => removed,
                    Err(error) => {
                        eprintln!("{TAG} failed to load blocked peer list: {error}");
                        reject_hello(peers, peer_id).await;
                        return;
                    }
                };
                if !removed.iter().any(|id| id == &new_record.device_id) {
                    let mut known = known_peer_records.lock().await;
                    *known = merge_peer_records(&known, &[new_record]);
                    if let Err(error) = save_known_peers(storage, &known).await {
                        eprintln!("{TAG} failed to persist known peers: {error}");
                        return;
                    }
                }
            }
        }

        // Reply hello
        let (reply_auth_nonce, reply_auth_hmac) = match secret.as_ref() {
            Some(secret) => {
                let nonce = generate_auth_nonce();
                let hmac = compute_hello_auth_hmac(secret, &my_space_id, &my_device_id, &nonce);
                (Some(nonce), Some(hmac))
            }
            None => (None, None),
        };
        let negotiated_capability_ids = negotiated
            .capabilities
            .iter()
            .map(|capability| capability.wire_id().to_owned())
            .collect::<Vec<_>>();
        let reply_negotiation_hmac = if peer_offer_authenticated {
            secret
                .as_deref()
                .zip(auth_nonce.as_deref())
                .zip(reply_auth_nonce.as_deref())
                .map(|((secret, peer_nonce), reply_nonce)| {
                    compute_negotiation_selection_hmac(
                        secret,
                        &ProtocolSelectionTranscript {
                            space_id: &my_space_id,
                            initiator_device_id: &peer_device_id,
                            responder_device_id: &my_device_id,
                            initiator_nonce: peer_nonce,
                            responder_nonce: reply_nonce,
                            initiator_offer: &peer_offer,
                            responder_offer: &local_offer,
                            selection: &negotiated,
                        },
                    )
                })
        } else {
            None
        };
        send_msg(
            &tx,
            &LanSyncMessage::Hello {
                protocol_version: negotiated.version,
                device_id: my_device_id,
                device_name: my_device_name,
                space_id: my_space_id,
                addresses: Some(my_addresses),
                auth_nonce: reply_auth_nonce,
                auth_hmac: reply_auth_hmac,
                platform: Some(crate::host::local_platform()),
                app_version: crate::host::app_version(),
                supported_protocol_versions: Some(local_offer.version_ids()),
                supported_capabilities: Some(local_offer.capability_ids()),
                negotiated_capabilities: Some(negotiated_capability_ids),
                negotiation_hmac: reply_negotiation_hmac,
            },
        );

        if let Some(handler) = on_peer_connect.lock().await.as_ref() {
            handler(peer_device_id);
        }

        // Send version vector
        let vector = match load_version_vector(storage).await {
            Ok(vector) => vector,
            Err(error) => {
                eprintln!("{TAG} failed to load sync version vector: {error}");
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

        // Send peer list after short delay
        let known = known_peer_records.lock().await.clone();
        let tx_delayed = tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            send_msg(&tx_delayed, &LanSyncMessage::PeerList { peers: known });
        });
    }
}

async fn send_protocol_rejection(
    peers: &Arc<Mutex<HashMap<usize, PeerState>>>,
    peer_id: usize,
    origin_device_id: &str,
    target_device_id: &str,
    error: ProtocolNegotiationError,
    offered_protocol_versions: &[u32],
) {
    let tx = peers.lock().await.get(&peer_id).map(|peer| peer.tx.clone());
    if let Some(tx) = tx {
        send_msg(
            &tx,
            &LanSyncMessage::ProtocolRejected {
                origin_device_id: origin_device_id.to_owned(),
                target_device_id: target_device_id.to_owned(),
                reason: error.code().to_owned(),
                supported_protocol_versions: SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
                offered_protocol_versions: if offered_protocol_versions.len()
                    <= MAX_PROTOCOL_OFFER_VERSIONS
                {
                    offered_protocol_versions.to_vec()
                } else {
                    Vec::new()
                },
            },
        );
    }
    reject_hello(peers, peer_id).await;
}
