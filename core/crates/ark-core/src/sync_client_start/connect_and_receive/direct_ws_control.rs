use super::*;

pub(super) enum DirectWsControl {
    Handled,
    CloseConnection,
    NotControl,
}

pub(super) async fn handle(
    message: &LanSyncMessage,
    tx: &ClientMessageSender,
    authenticated: bool,
    on_peer_list: &Arc<Mutex<Option<ClientOnPeerListCallback>>>,
    local_device_id: &str,
    authenticated_peer_id: &str,
) -> DirectWsControl {
    match message {
        LanSyncMessage::Ping { ts } => {
            let _ = send_msg(tx, LanSyncMessage::Pong { ts: *ts }).await;
            DirectWsControl::Handled
        }
        LanSyncMessage::Pong { .. }
        | LanSyncMessage::PairingRejected { .. }
        | LanSyncMessage::PairingAccepted { .. } => DirectWsControl::Handled,
        LanSyncMessage::PeerList { peers } => {
            if authenticated {
                if let Some(handler) = on_peer_list.lock().await.as_ref() {
                    handler(peers.clone());
                }
            }
            DirectWsControl::Handled
        }
        LanSyncMessage::RegistryAdmissionStart { .. }
        | LanSyncMessage::RegistryAdmissionProof { .. }
        | LanSyncMessage::RegistryAdmissionConfirm { .. }
        | LanSyncMessage::RegistrySyncStart { .. }
        | LanSyncMessage::RegistryEventsPage { .. }
        | LanSyncMessage::RegistrySyncReady { .. }
        | LanSyncMessage::RegistryEventUpdate { .. } => DirectWsControl::NotControl,
        LanSyncMessage::ProtocolRejected {
            origin_device_id,
            target_device_id,
            reason,
            ..
        } => {
            if target_device_id == local_device_id
                && (authenticated_peer_id.is_empty() || origin_device_id == authenticated_peer_id)
            {
                eprintln!("{TAG} Peer rejected protocol: {reason}");
                DirectWsControl::CloseConnection
            } else {
                DirectWsControl::Handled
            }
        }
        _ => DirectWsControl::NotControl,
    }
}
