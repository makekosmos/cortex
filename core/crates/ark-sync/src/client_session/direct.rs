mod messages;
pub use messages::{DirectClientMessage, DirectClientSessionEffect, DirectClientSessionInput};

pub struct DirectClientSession {
    phase: Phase,
    peer_device_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    AwaitingHello,
    AwaitingRegistryAdmission,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectClientSessionRejection {
    RepeatedHello,
    SelfConnection,
    RegistryControlBeforeHello,
}

impl DirectClientSession {
    pub fn new() -> Self {
        Self {
            phase: Phase::AwaitingHello,
            peer_device_id: None,
        }
    }

    pub fn accept_server_hello(
        &mut self,
        peer_device_id: &str,
        local_device_id: &str,
    ) -> Result<(), DirectClientSessionRejection> {
        if self.phase != Phase::AwaitingHello {
            return Err(DirectClientSessionRejection::RepeatedHello);
        }
        if !peer_device_id.is_empty() && peer_device_id == local_device_id {
            return Err(DirectClientSessionRejection::SelfConnection);
        }
        self.peer_device_id = Some(peer_device_id.to_owned());
        self.phase = Phase::AwaitingRegistryAdmission;
        Ok(())
    }

    pub async fn receive(
        &self,
        session_io: &mut crate::session_io::SessionIo,
    ) -> DirectClientSessionInput {
        match session_io.receive().await {
            Some(crate::session_io::SessionIoEvent::Message(message)) => {
                match self.transition(*message) {
                    DirectClientSessionEffect::Dispatch(message) => {
                        DirectClientSessionInput::Message(message)
                    }
                    DirectClientSessionEffect::Ignore => DirectClientSessionInput::Ignore,
                    DirectClientSessionEffect::Close(reason) => {
                        DirectClientSessionInput::Rejected(reason)
                    }
                }
            }
            Some(crate::session_io::SessionIoEvent::MalformedMessage) => {
                DirectClientSessionInput::Malformed
            }
            Some(crate::session_io::SessionIoEvent::Failed(error)) => {
                DirectClientSessionInput::Failed(error)
            }
            Some(crate::session_io::SessionIoEvent::Closed) | None => {
                DirectClientSessionInput::Closed
            }
        }
    }

    pub fn mark_ready(&mut self) -> bool {
        if self.phase != Phase::AwaitingRegistryAdmission {
            return false;
        }
        self.phase = Phase::Ready;
        true
    }

    pub fn is_ready(&self) -> bool {
        self.phase == Phase::Ready
    }

    pub fn peer_device_id(&self) -> &str {
        self.peer_device_id.as_deref().unwrap_or_default()
    }
}

impl Default for DirectClientSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::LanSyncMessage;

    fn hello(device_id: &str) -> LanSyncMessage {
        LanSyncMessage::Hello {
            protocol_version: 1,
            device_id: device_id.into(),
            device_name: "peer".into(),
            space_id: "space".into(),
            addresses: None,
            auth_nonce: None,
            auth_hmac: None,
            platform: None,
            app_version: None,
            supported_protocol_versions: None,
            supported_capabilities: None,
            negotiated_capabilities: None,
            negotiation_hmac: None,
        }
    }

    fn sync_changes() -> LanSyncMessage {
        LanSyncMessage::SyncChanges {
            batch_id: "batch".into(),
            entities: Vec::new(),
            is_last: true,
            policy_identity: None,
            origin_device_id: None,
            usage_complete_through: None,
        }
    }

    #[test]
    fn client_session_becomes_ready_only_after_registry_admission() {
        let mut session = DirectClientSession::new();
        assert!(!session.mark_ready());
        assert!(!session.is_ready());
        session
            .accept_server_hello("peer-device", "local-device")
            .unwrap();

        assert_eq!(session.peer_device_id(), "peer-device");
        assert!(!session.is_ready());
        assert!(session.mark_ready());
        assert!(session.is_ready());
        assert!(!session.mark_ready());
    }

    #[test]
    fn client_session_rejects_repeated_hello_and_self_connection() {
        let mut session = DirectClientSession::new();
        session
            .accept_server_hello("peer-device", "local-device")
            .unwrap();

        assert_eq!(
            session.accept_server_hello("peer-device", "local-device"),
            Err(DirectClientSessionRejection::RepeatedHello)
        );
        assert_eq!(session.peer_device_id(), "peer-device");

        let mut self_session = DirectClientSession::new();
        assert_eq!(
            self_session.accept_server_hello("local-device", "local-device"),
            Err(DirectClientSessionRejection::SelfConnection)
        );
        assert_eq!(self_session.peer_device_id(), "");
    }

    #[test]
    fn client_session_classifies_messages_and_gates_sync_until_registry_admission() {
        let mut session = DirectClientSession::new();
        assert!(matches!(
            session.transition(LanSyncMessage::VersionVector {
                vector: Default::default(),
                origin_device_id: None,
            }),
            DirectClientSessionEffect::Ignore
        ));
        assert!(matches!(
            session.transition(LanSyncMessage::SyncPolicy {
                revision: 1,
                digest: "digest".into(),
                active: true,
                rules: Vec::new(),
            }),
            DirectClientSessionEffect::Ignore
        ));
        assert!(matches!(
            session.transition(LanSyncMessage::LiveAck {
                change_id: "change".into(),
                policy_identity: None,
            }),
            DirectClientSessionEffect::Ignore
        ));
        assert!(matches!(
            session.transition(LanSyncMessage::PeerList { peers: Vec::new() }),
            DirectClientSessionEffect::Ignore
        ));
        assert!(matches!(
            session.transition(LanSyncMessage::RegistrySyncStart {
                session_id: "registry-session".into(),
            }),
            DirectClientSessionEffect::Close(
                DirectClientSessionRejection::RegistryControlBeforeHello
            )
        ));
        assert!(matches!(
            session.transition(LanSyncMessage::Ping { ts: 5 }),
            DirectClientSessionEffect::Dispatch(message)
                if matches!(*message, DirectClientMessage::Ping { ts: 5 })
        ));
        assert!(matches!(
            session.transition(hello("peer-device")),
            DirectClientSessionEffect::Dispatch(message)
                if matches!(*message, DirectClientMessage::Hello(_))
        ));
        session
            .accept_server_hello("peer-device", "local-device")
            .unwrap();

        assert!(matches!(
            session.transition(sync_changes()),
            DirectClientSessionEffect::Ignore
        ));
        assert!(matches!(
            session.transition(hello("peer-device")),
            DirectClientSessionEffect::Close(DirectClientSessionRejection::RepeatedHello)
        ));
        assert!(matches!(
            session.transition(LanSyncMessage::RegistrySyncStart {
                session_id: "registry-session".into(),
            }),
            DirectClientSessionEffect::Dispatch(message)
                if matches!(*message, DirectClientMessage::RegistryControl(_))
        ));
        assert!(session.mark_ready());
        assert!(matches!(
            session.transition(LanSyncMessage::VersionVector {
                vector: Default::default(),
                origin_device_id: None,
            }),
            DirectClientSessionEffect::Dispatch(message)
                if matches!(*message, DirectClientMessage::VersionVector(_))
        ));
    }
}
