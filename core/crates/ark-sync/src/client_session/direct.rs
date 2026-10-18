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
}
