use super::{
    accept_client_hello, AcceptedClientHello, ClientHelloRejection, ResponderHelloRequest,
};
use crate::protocol::LanSyncMessage;

pub struct DirectServerSession {
    request: ResponderHelloRequest,
    phase: DirectServerHelloPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectServerSyncStartPolicy {
    Immediate,
    AfterPolicyPersistence,
}

enum DirectServerHelloPhase {
    AwaitingHello,
    CheckingRemovedPeer(Box<AcceptedClientHello>),
    AwaitingRegistryAdmission,
    AwaitingPolicy,
    PersistingPolicy,
    Ready,
    Closed,
}

pub enum DirectServerHelloInput {
    Message(Box<LanSyncMessage>),
    RemovedPeerResult(Result<bool, String>),
}

pub enum DirectServerHelloEffect {
    CheckRemovedPeer { device_id: String },
    Accepted(Box<AcceptedClientHello>),
    Rejected(ClientHelloRejection),
    Close(DirectServerHelloCloseReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectServerHelloCloseReason {
    SelfConnection,
    RemovedPeer,
    RemovedPeerLookupFailed,
    RepeatedHello,
    UnexpectedInput,
    AlreadyClosed,
}

pub struct DirectServerHelloTransition {
    pub effect: DirectServerHelloEffect,
}

impl DirectServerSession {
    pub fn new(request: ResponderHelloRequest) -> Self {
        Self {
            request,
            phase: DirectServerHelloPhase::AwaitingHello,
        }
    }

    pub fn mark_ready(&mut self, sync_start_policy: DirectServerSyncStartPolicy) -> bool {
        if !matches!(
            self.phase,
            DirectServerHelloPhase::AwaitingRegistryAdmission
        ) {
            return false;
        }
        self.phase = match sync_start_policy {
            DirectServerSyncStartPolicy::Immediate => DirectServerHelloPhase::Ready,
            DirectServerSyncStartPolicy::AfterPolicyPersistence => {
                DirectServerHelloPhase::AwaitingPolicy
            }
        };
        true
    }

    pub fn start_sync_after_policy_persisted(&mut self) -> bool {
        if !matches!(self.phase, DirectServerHelloPhase::PersistingPolicy) {
            return false;
        }
        self.phase = DirectServerHelloPhase::Ready;
        true
    }

    pub fn begin_policy_persistence(&mut self) -> bool {
        if !matches!(self.phase, DirectServerHelloPhase::AwaitingPolicy) {
            return false;
        }
        self.phase = DirectServerHelloPhase::PersistingPolicy;
        true
    }

    pub fn transition(&mut self, input: DirectServerHelloInput) -> DirectServerHelloTransition {
        let phase = std::mem::replace(&mut self.phase, DirectServerHelloPhase::Closed);
        let effect = match (phase, input) {
            (DirectServerHelloPhase::AwaitingHello, DirectServerHelloInput::Message(message)) => {
                match accept_client_hello(*message, self.request.clone()) {
                    Ok(accepted)
                        if !accepted.peer_device_id.is_empty()
                            && accepted.peer_device_id == self.request.device_id =>
                    {
                        DirectServerHelloEffect::Close(DirectServerHelloCloseReason::SelfConnection)
                    }
                    Ok(accepted) => {
                        let device_id = accepted.peer_device_id.clone();
                        self.phase =
                            DirectServerHelloPhase::CheckingRemovedPeer(Box::new(accepted));
                        DirectServerHelloEffect::CheckRemovedPeer { device_id }
                    }
                    Err(rejection) => DirectServerHelloEffect::Rejected(rejection),
                }
            }
            (
                DirectServerHelloPhase::CheckingRemovedPeer(accepted),
                DirectServerHelloInput::RemovedPeerResult(Ok(false)),
            ) => {
                self.phase = DirectServerHelloPhase::AwaitingRegistryAdmission;
                DirectServerHelloEffect::Accepted(accepted)
            }
            (
                DirectServerHelloPhase::CheckingRemovedPeer(_),
                DirectServerHelloInput::RemovedPeerResult(Ok(true)),
            ) => DirectServerHelloEffect::Close(DirectServerHelloCloseReason::RemovedPeer),
            (
                DirectServerHelloPhase::CheckingRemovedPeer(_),
                DirectServerHelloInput::RemovedPeerResult(Err(_)),
            ) => DirectServerHelloEffect::Close(
                DirectServerHelloCloseReason::RemovedPeerLookupFailed,
            ),
            (
                DirectServerHelloPhase::CheckingRemovedPeer(_),
                DirectServerHelloInput::Message(_),
            ) => DirectServerHelloEffect::Close(DirectServerHelloCloseReason::RepeatedHello),
            (
                DirectServerHelloPhase::AwaitingHello,
                DirectServerHelloInput::RemovedPeerResult(_),
            ) => DirectServerHelloEffect::Close(DirectServerHelloCloseReason::UnexpectedInput),
            (
                phase @ DirectServerHelloPhase::AwaitingRegistryAdmission,
                DirectServerHelloInput::Message(_),
            ) => {
                self.phase = phase;
                DirectServerHelloEffect::Close(DirectServerHelloCloseReason::RepeatedHello)
            }
            (phase @ DirectServerHelloPhase::AwaitingRegistryAdmission, _) => {
                self.phase = phase;
                DirectServerHelloEffect::Close(DirectServerHelloCloseReason::UnexpectedInput)
            }
            (phase @ DirectServerHelloPhase::AwaitingPolicy, _) => {
                self.phase = phase;
                DirectServerHelloEffect::Close(DirectServerHelloCloseReason::UnexpectedInput)
            }
            (phase @ DirectServerHelloPhase::PersistingPolicy, _) => {
                self.phase = phase;
                DirectServerHelloEffect::Close(DirectServerHelloCloseReason::UnexpectedInput)
            }
            (phase @ DirectServerHelloPhase::Ready, _) => {
                self.phase = phase;
                DirectServerHelloEffect::Close(DirectServerHelloCloseReason::UnexpectedInput)
            }
            (DirectServerHelloPhase::Closed, _) => {
                DirectServerHelloEffect::Close(DirectServerHelloCloseReason::AlreadyClosed)
            }
        };
        DirectServerHelloTransition { effect }
    }
}
