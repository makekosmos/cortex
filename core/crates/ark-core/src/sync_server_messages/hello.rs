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
        task_merge_bootstrap_v2,
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
            .is_some_and(|peer| peer.negotiated_protocol.is_some());
        if repeated {
            eprintln!("{TAG} Rejecting repeated hello on a connection");
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
        let mut local_offer = ProtocolOffer::current_with_registry_history(true);
        if *task_merge_bootstrap_v2 {
            local_offer = local_offer.with_task_merge_bootstrap_v2();
        }
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
        if !negotiated
            .capabilities
            .contains(&crate::protocol::ProtocolCapability::RegistryHistoryV1)
        {
            send_protocol_rejection(
                peers,
                peer_id,
                &my_device_id,
                &peer_device_id,
                ProtocolNegotiationError::MissingRegistryHistory,
                &peer_offer.version_ids(),
            )
            .await;
            return;
        }

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

        let admission = match ark_sync::registry_admission::DirectRegistryAdmission::responder(
            &my_device_id,
            &peer_device_id,
            &my_space_id,
            negotiated.clone(),
        ) {
            Ok(admission) => admission,
            Err(error) => {
                eprintln!("{TAG} invalid direct registry admission: {error}");
                reject_hello(peers, peer_id).await;
                return;
            }
        };

        let tx = {
            let mut peers_guard = peers.lock().await;
            if let Some(peer) = peers_guard.get_mut(&peer_id) {
                peer.device_id = peer_device_id.clone();
                peer.device_name = peer_device_name.clone();
                peer.addresses = addresses.clone().unwrap_or_default();
                peer.platform = peer_platform.clone();
                peer.app_version = peer_app_version.clone();
                peer.negotiated_protocol = Some(negotiated.clone());
                peer.registry_admission = Some(admission);
                peer.tx.clone()
            } else {
                return;
            }
        };

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
                platform: Some(ark_net::host::local_platform()),
                app_version: ark_net::host::app_version(),
                supported_protocol_versions: Some(local_offer.version_ids()),
                supported_capabilities: Some(local_offer.capability_ids()),
                negotiated_capabilities: Some(negotiated_capability_ids),
                negotiation_hmac: reply_negotiation_hmac,
            },
        );
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
