use super::*;

pub(super) async fn handle_hello(ctx: &MessageContext, peer_id: usize, msg: LanSyncMessage) {
    let MessageContext {
        peers,
        space_id,
        device_id,
        device_name,
        own_addresses,
        auth_secret,
        task_merge_bootstrap_v2,
        ..
    } = ctx;
    let my_device_id = device_id.read().await.clone();
    let request = ark_sync::server_session::ResponderHelloRequest {
        device_id: my_device_id.clone(),
        device_name: device_name.read().await.clone(),
        space_id: space_id.read().await.clone(),
        addresses: own_addresses.read().await.clone(),
        auth_secret: auth_secret.read().await.clone(),
        task_merge_bootstrap_v2: *task_merge_bootstrap_v2,
        platform: Some(ark_net::host::local_platform()),
        app_version: ark_net::host::app_version(),
    };
    let my_space_id = request.space_id.clone();
    let effect = {
        let mut peers = peers.lock().await;
        let Some(peer) = peers.get_mut(&peer_id) else {
            return;
        };
        let session = peer
            .server_session
            .get_or_insert_with(|| ark_sync::server_session::DirectServerSession::new(request));
        session
            .transition(ark_sync::server_session::DirectServerHelloInput::Message(
                Box::new(msg),
            ))
            .effect
    };
    apply_hello_effect(ctx, peer_id, &my_device_id, &my_space_id, effect).await;
}

async fn apply_hello_effect(
    ctx: &MessageContext,
    peer_id: usize,
    my_device_id: &str,
    my_space_id: &str,
    mut effect: ark_sync::server_session::DirectServerHelloEffect,
) {
    use ark_sync::server_session::{
        DirectServerHelloCloseReason as CloseReason, DirectServerHelloEffect as Effect,
        DirectServerHelloInput as Input,
    };

    loop {
        match effect {
            Effect::CheckRemovedPeer { device_id } => {
                let result = match load_removed_peer_ids(&ctx.storage).await {
                    Ok(removed) => Ok(!device_id.is_empty()
                        && removed.iter().any(|removed_id| removed_id == &device_id)),
                    Err(error) => {
                        eprintln!("{TAG} failed to load blocked peer list: {error}");
                        Err(error)
                    }
                };
                // Connection ids are monotonically allocated. Recheck membership after the
                // storage await so a completion cannot revive a peer already removed.
                effect = {
                    let mut peers = ctx.peers.lock().await;
                    let Some(peer) = peers.get_mut(&peer_id) else {
                        return;
                    };
                    let Some(session) = peer.server_session.as_mut() else {
                        return;
                    };
                    session.transition(Input::RemovedPeerResult(result)).effect
                };
                continue;
            }
            Effect::Accepted(accepted) => {
                let ark_sync::server_session::AcceptedClientHello {
                    peer_device_id,
                    peer_device_name,
                    peer_addresses,
                    peer_platform,
                    peer_app_version,
                    protocol: negotiated,
                    reply,
                } = *accepted;
                let admission = match ark_sync::server_session::DirectServerAdmission::begin(
                    my_device_id,
                    &peer_device_id,
                    my_space_id,
                    negotiated.clone(),
                ) {
                    Ok(admission) => admission,
                    Err(error) => {
                        eprintln!("{TAG} invalid direct registry admission: {error}");
                        reject_hello(&ctx.peers, peer_id).await;
                        return;
                    }
                };
                let tx = {
                    let mut peers = ctx.peers.lock().await;
                    let Some(peer) = peers.get_mut(&peer_id) else {
                        return;
                    };
                    peer.device_id = peer_device_id;
                    peer.device_name = peer_device_name;
                    peer.addresses = peer_addresses;
                    peer.platform = peer_platform;
                    peer.app_version = peer_app_version;
                    peer.negotiated_protocol = Some(negotiated);
                    peer.registry_admission = Some(admission);
                    peer.tx.clone()
                };
                send_msg(&tx, &reply).await;
                return;
            }
            Effect::Rejected(rejection) => {
                match rejection {
                    ark_sync::server_session::ClientHelloRejection::DifferentSpace => {
                        eprintln!("{TAG} Rejecting peer hello from a different space");
                        reject_hello(&ctx.peers, peer_id).await;
                    }
                    ark_sync::server_session::ClientHelloRejection::InvalidAuthentication => {
                        eprintln!("{TAG} Rejecting peer hello with invalid HMAC");
                        reject_hello(&ctx.peers, peer_id).await;
                    }
                    ark_sync::server_session::ClientHelloRejection::Protocol {
                        peer_device_id,
                        error,
                        offered_protocol_versions,
                    } => {
                        send_protocol_rejection(
                            &ctx.peers,
                            peer_id,
                            my_device_id,
                            &peer_device_id,
                            error,
                            &offered_protocol_versions,
                        )
                        .await;
                    }
                }
                return;
            }
            Effect::Close(reason) => {
                match reason {
                    CloseReason::SelfConnection => {
                        eprintln!("{TAG} Rejecting self-connect on a direct sync session");
                    }
                    CloseReason::RemovedPeer => {
                        eprintln!("{TAG} Rejecting blocked peer on a direct sync session");
                    }
                    CloseReason::RemovedPeerLookupFailed => {}
                    CloseReason::RepeatedHello => {
                        eprintln!("{TAG} Rejecting repeated hello on a connection");
                    }
                    CloseReason::UnexpectedInput | CloseReason::AlreadyClosed => {
                        eprintln!("{TAG} Closing invalid direct sync handshake");
                    }
                }
                reject_hello(&ctx.peers, peer_id).await;
                return;
            }
        }
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
        )
        .await;
    }
    reject_hello(peers, peer_id).await;
}
