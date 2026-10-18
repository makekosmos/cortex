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
    let accepted = match ark_sync::server_session::accept_client_hello(
        msg,
        ark_sync::server_session::ResponderHelloRequest {
            device_id: my_device_id.clone(),
            device_name: my_device_name,
            space_id: my_space_id.clone(),
            addresses: my_addresses,
            auth_secret: auth_secret.read().await.clone(),
            task_merge_bootstrap_v2: *task_merge_bootstrap_v2,
            platform: Some(ark_net::host::local_platform()),
            app_version: ark_net::host::app_version(),
        },
    ) {
        Ok(accepted) => accepted,
        Err(ark_sync::server_session::ClientHelloRejection::DifferentSpace) => {
            eprintln!("{TAG} Rejecting peer hello from a different space");
            reject_hello(peers, peer_id).await;
            return;
        }
        Err(ark_sync::server_session::ClientHelloRejection::InvalidAuthentication) => {
            eprintln!("{TAG} Rejecting peer hello with invalid HMAC");
            reject_hello(peers, peer_id).await;
            return;
        }
        Err(ark_sync::server_session::ClientHelloRejection::Protocol {
            peer_device_id,
            error,
            offered_protocol_versions,
        }) => {
            send_protocol_rejection(
                peers,
                peer_id,
                &my_device_id,
                &peer_device_id,
                error,
                &offered_protocol_versions,
            )
            .await;
            return;
        }
    };
    let ark_sync::server_session::AcceptedClientHello {
        peer_device_id,
        peer_device_name,
        peer_addresses,
        peer_platform,
        peer_app_version,
        protocol: negotiated,
        reply,
    } = accepted;

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

    let admission = match ark_sync::server_session::DirectServerAdmission::begin(
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
            peer.device_name = peer_device_name;
            peer.addresses = peer_addresses;
            peer.platform = peer_platform;
            peer.app_version = peer_app_version;
            peer.negotiated_protocol = Some(negotiated);
            peer.registry_admission = Some(admission);
            peer.tx.clone()
        } else {
            return;
        }
    };

    send_msg(&tx, &reply).await;
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
