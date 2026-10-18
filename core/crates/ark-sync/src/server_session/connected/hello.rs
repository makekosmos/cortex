use super::super::{
    ClientHelloRejection, DirectServerAdmission, DirectServerSession, DirectServerSessionEffect,
    DirectServerSessionInput, ServerPeerProjection, ServerPeerReadiness, ServerSessionActor,
};
use super::ConnectedServerSession;
use crate::protocol::{LanSyncMessage, ProtocolNegotiationError};

impl ConnectedServerSession {
    pub(in crate::server_session) async fn handle_hello(
        &mut self,
        actor: &ServerSessionActor,
        message: LanSyncMessage,
        sender: &crate::session_io::SessionIoSender,
    ) -> bool {
        let direct = self
            .direct
            .get_or_insert_with(|| DirectServerSession::new(self.request.clone()));
        let mut effect = direct
            .transition(DirectServerSessionInput::Message(Box::new(message)))
            .effect;
        loop {
            if actor.is_revoked() {
                return false;
            }
            match effect {
                DirectServerSessionEffect::CheckRemovedPeer { device_id } => {
                    let result = self.storage.load_removed_peer_ids().await.map(|removed| {
                        !device_id.is_empty() && removed.iter().any(|value| value == &device_id)
                    });
                    if actor.is_revoked() {
                        return false;
                    }
                    effect = direct
                        .transition(DirectServerSessionInput::RemovedPeerResult(result))
                        .effect;
                }
                DirectServerSessionEffect::Accepted(accepted) => {
                    let accepted = *accepted;
                    let admission = match DirectServerAdmission::begin(
                        &self.request.device_id,
                        &accepted.peer_device_id,
                        &self.request.space_id,
                        accepted.protocol.clone(),
                    ) {
                        Ok(admission) => admission,
                        Err(_) => {
                            sender.close_after_flush();
                            return false;
                        }
                    };
                    self.peer = ServerPeerProjection {
                        device_id: Some(accepted.peer_device_id),
                        device_name: Some(accepted.peer_device_name),
                        addresses: accepted.peer_addresses,
                        platform: accepted.peer_platform,
                        app_version: accepted.peer_app_version,
                        registry_session_id: None,
                        readiness: ServerPeerReadiness::Pending,
                    };
                    self.protocol = Some(accepted.protocol);
                    self.admission = Some(admission);
                    if actor.is_revoked() {
                        return false;
                    }
                    if sender.send(accepted.reply).await.is_err() || actor.is_revoked() {
                        return false;
                    }
                    actor.publish_projection(self.peer.clone());
                    return true;
                }
                DirectServerSessionEffect::Rejected(rejection) => {
                    if let ClientHelloRejection::Protocol {
                        peer_device_id,
                        error,
                        offered_protocol_versions,
                    } = rejection
                    {
                        let message = protocol_rejection(
                            &self.request.device_id,
                            &peer_device_id,
                            error,
                            &offered_protocol_versions,
                        );
                        if sender.send(message).await.is_ok() && !actor.is_revoked() {
                            sender.close_after_flush();
                        }
                    } else {
                        sender.close_after_flush();
                    }
                    return false;
                }
                DirectServerSessionEffect::Close(reason) => {
                    let _ = reason;
                    sender.close_after_flush();
                    return false;
                }
                DirectServerSessionEffect::Dispatch(_) | DirectServerSessionEffect::Ignore => {
                    sender.close_after_flush();
                    return false;
                }
            }
        }
    }
}

fn protocol_rejection(
    origin_device_id: &str,
    target_device_id: &str,
    error: ProtocolNegotiationError,
    offered_protocol_versions: &[u32],
) -> LanSyncMessage {
    LanSyncMessage::ProtocolRejected {
        origin_device_id: origin_device_id.to_owned(),
        target_device_id: target_device_id.to_owned(),
        reason: error.code().to_owned(),
        supported_protocol_versions: crate::protocol::SUPPORTED_PROTOCOL_VERSIONS.to_vec(),
        offered_protocol_versions: if offered_protocol_versions.len()
            <= crate::protocol::MAX_PROTOCOL_OFFER_VERSIONS
        {
            offered_protocol_versions.to_vec()
        } else {
            Vec::new()
        },
    }
}
