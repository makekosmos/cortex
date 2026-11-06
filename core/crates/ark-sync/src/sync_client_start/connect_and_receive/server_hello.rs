use super::super::{send_msg, ClientMessageSender, TAG};
use super::registry_admission::PendingRegistryAdmission;
use ark_domain::sync_contracts::protocol_negotiation::ProtocolOffer;
use ark_domain::sync_contracts::wire::LanSyncMessage;
use ark_domain::types::PeerRecord;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::RwLock;

pub(super) struct Context<'a> {
    pub(super) local_device_id: &'a str,
    pub(super) local_space_id: &'a str,
    pub(super) auth_secret: Option<&'a str>,
    pub(super) local_offer: &'a ProtocolOffer,
    pub(super) local_auth_nonce: Option<&'a str>,
    pub(super) peer: &'a Arc<RwLock<PeerRecord>>,
    pub(super) winning_addr: &'a str,
    pub(super) stopped: &'a AtomicBool,
    pub(super) tx: &'a ClientMessageSender,
    pub(super) client_session: &'a mut crate::client_session::DirectClientSession,
    pub(super) pending_registry_admission: &'a mut Option<PendingRegistryAdmission>,
}

pub(super) async fn handle(hello: LanSyncMessage, context: Context<'_>) -> bool {
    let Context {
        local_device_id,
        local_space_id,
        auth_secret,
        local_offer,
        local_auth_nonce,
        peer,
        winning_addr,
        stopped,
        tx,
        client_session,
        pending_registry_admission,
    } = context;
    let accepted = match crate::client_session::accept_server_hello(
        hello,
        crate::client_session::ServerHelloValidationRequest {
            local_device_id,
            local_space_id,
            auth_secret,
            local_offer,
            local_auth_nonce,
        },
    ) {
        Ok(accepted) => accepted,
        Err(error) => {
            eprintln!("{TAG} {error}");
            return false;
        }
    };
    let crate::client_session::AcceptedServerHello {
        device_id: server_device_id,
        device_name: server_device_name,
        platform: server_platform,
        app_version: server_app_version,
        protocol: negotiated_protocol,
    } = accepted;
    match client_session.accept_server_hello(&server_device_id, local_device_id) {
        Ok(()) => {}
        Err(crate::client_session::DirectClientSessionRejection::SelfConnection) => {
            eprintln!("{TAG} Rejecting self-connect to {server_device_name} ({server_device_id})");
            let remaining = {
                let mut peer = peer.write().await;
                peer.addresses.retain(|address| address != winning_addr);
                peer.last_address = None;
                peer.addresses.len()
            };
            if remaining == 0 {
                stopped.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            return false;
        }
        Err(crate::client_session::DirectClientSessionRejection::RepeatedHello) => {
            eprintln!("{TAG} rejected Hello: repeated session");
            return false;
        }
        Err(crate::client_session::DirectClientSessionRejection::RegistryControlBeforeHello) => {
            eprintln!("{TAG} invalid Hello session phase");
            return false;
        }
    }

    tx.set_authenticated_peer_id(server_device_id.to_owned())
        .await;
    let pending_peer = PeerRecord {
        device_id: server_device_id.to_owned(),
        device_name: server_device_name,
        addresses: Vec::new(),
        last_seen: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        last_address: Some(winning_addr.to_owned()),
        platform: server_platform,
        app_version: server_app_version,
    };
    let (admission, start) = match PendingRegistryAdmission::begin(
        local_device_id,
        &server_device_id,
        local_space_id,
        negotiated_protocol,
        pending_peer,
    ) {
        Ok(admission) => admission,
        Err(error) => {
            eprintln!("{TAG} admission start failed: {error}");
            return false;
        }
    };
    *pending_registry_admission = Some(admission);
    if let Err(error) = send_msg(tx, start).await {
        eprintln!("{TAG} registry admission send failed: {error}");
        return false;
    }
    true
}
