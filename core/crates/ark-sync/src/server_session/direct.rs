use super::{
    accept_client_hello, AcceptedClientHello, ClientHelloRejection, ResponderHelloRequest,
};
use ark_sync_contracts::wire::{requires_registry_admission, LanSyncMessage};

pub enum DirectServerMessage {
    SyncPolicy(crate::client_session::SyncPolicyUpdate),
    VersionVector(ark_domain::types::VersionVector),
    SyncChanges(DirectServerSyncChanges),
    RegistryAdmission(LanSyncMessage),
    TransportControl(LanSyncMessage),
    Live(LanSyncMessage),
}

pub struct DirectServerSyncChanges {
    pub batch_id: String,
    pub entities: Vec<ark_domain::types::SyncEntity>,
    pub is_last: bool,
    pub policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    pub usage_complete_through: Option<std::collections::HashMap<String, u64>>,
}

pub fn classify_server_message(message: LanSyncMessage) -> DirectServerMessage {
    match message {
        LanSyncMessage::SyncPolicy {
            revision,
            digest,
            active,
            rules,
        } => DirectServerMessage::SyncPolicy(crate::client_session::SyncPolicyUpdate {
            revision,
            digest,
            active,
            rules,
        }),
        LanSyncMessage::VersionVector { vector, .. } => DirectServerMessage::VersionVector(vector),
        LanSyncMessage::SyncChanges {
            batch_id,
            entities,
            is_last,
            policy_identity,
            usage_complete_through,
            ..
        } => DirectServerMessage::SyncChanges(DirectServerSyncChanges {
            batch_id,
            entities,
            is_last,
            policy_identity,
            usage_complete_through,
        }),
        message @ (LanSyncMessage::RegistryAdmissionStart { .. }
        | LanSyncMessage::RegistryAdmissionProof { .. }
        | LanSyncMessage::RegistryAdmissionConfirm { .. }
        | LanSyncMessage::RegistrySyncStart { .. }
        | LanSyncMessage::RegistryEventsPage { .. }
        | LanSyncMessage::RegistrySyncReady { .. }
        | LanSyncMessage::RegistryEventUpdate { .. }) => {
            DirectServerMessage::RegistryAdmission(message)
        }
        message @ (LanSyncMessage::PairingAccepted { .. }
        | LanSyncMessage::PairingRejected { .. }) => DirectServerMessage::TransportControl(message),
        message => DirectServerMessage::Live(message),
    }
}

pub struct DirectServerSession {
    request: ResponderHelloRequest,
    phase: DirectServerSessionPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectServerSyncStartPolicy {
    Immediate,
    AfterPolicyPersistence,
}

enum DirectServerSessionPhase {
    AwaitingHello,
    CheckingRemovedPeer(Box<AcceptedClientHello>),
    AwaitingRegistryAdmission,
    AwaitingPolicy,
    PersistingPolicy,
    Ready,
    Closed,
}

pub enum DirectServerSessionInput {
    Message(Box<LanSyncMessage>),
    RemovedPeerResult(Result<bool, String>),
}

pub enum DirectServerSessionEffect {
    CheckRemovedPeer { device_id: String },
    Accepted(Box<AcceptedClientHello>),
    Rejected(ClientHelloRejection),
    Dispatch(Box<DirectServerMessage>),
    Ignore,
    Close(DirectServerSessionCloseReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectServerSessionCloseReason {
    SelfConnection,
    RemovedPeer,
    RemovedPeerLookupFailed,
    RepeatedHello,
    UnexpectedInput,
    AlreadyClosed,
}

pub struct DirectServerSessionTransition {
    pub effect: DirectServerSessionEffect,
}

impl DirectServerSession {
    pub fn new(request: ResponderHelloRequest) -> Self {
        Self {
            request,
            phase: DirectServerSessionPhase::AwaitingHello,
        }
    }

    pub fn mark_ready(&mut self, sync_start_policy: DirectServerSyncStartPolicy) -> bool {
        if !matches!(
            self.phase,
            DirectServerSessionPhase::AwaitingRegistryAdmission
        ) {
            return false;
        }
        self.phase = match sync_start_policy {
            DirectServerSyncStartPolicy::Immediate => DirectServerSessionPhase::Ready,
            DirectServerSyncStartPolicy::AfterPolicyPersistence => {
                DirectServerSessionPhase::AwaitingPolicy
            }
        };
        true
    }

    pub fn start_sync_after_policy_persisted(&mut self) -> bool {
        if !matches!(self.phase, DirectServerSessionPhase::PersistingPolicy) {
            return false;
        }
        self.phase = DirectServerSessionPhase::Ready;
        true
    }

    pub fn transition(&mut self, input: DirectServerSessionInput) -> DirectServerSessionTransition {
        let phase = std::mem::replace(&mut self.phase, DirectServerSessionPhase::Closed);
        let effect = match (phase, input) {
            (
                DirectServerSessionPhase::AwaitingHello,
                DirectServerSessionInput::Message(message),
            ) => {
                if !matches!(message.as_ref(), LanSyncMessage::Hello { .. }) {
                    self.phase = DirectServerSessionPhase::AwaitingHello;
                    return DirectServerSessionTransition {
                        effect: if requires_registry_admission(message.as_ref()) {
                            DirectServerSessionEffect::Ignore
                        } else {
                            DirectServerSessionEffect::Dispatch(Box::new(classify_server_message(
                                *message,
                            )))
                        },
                    };
                }
                match accept_client_hello(*message, self.request.clone()) {
                    Ok(accepted)
                        if !accepted.peer_device_id.is_empty()
                            && accepted.peer_device_id == self.request.device_id =>
                    {
                        DirectServerSessionEffect::Close(
                            DirectServerSessionCloseReason::SelfConnection,
                        )
                    }
                    Ok(accepted) => {
                        let device_id = accepted.peer_device_id.clone();
                        self.phase =
                            DirectServerSessionPhase::CheckingRemovedPeer(Box::new(accepted));
                        DirectServerSessionEffect::CheckRemovedPeer { device_id }
                    }
                    Err(rejection) => DirectServerSessionEffect::Rejected(rejection),
                }
            }
            (
                DirectServerSessionPhase::CheckingRemovedPeer(accepted),
                DirectServerSessionInput::RemovedPeerResult(Ok(false)),
            ) => {
                self.phase = DirectServerSessionPhase::AwaitingRegistryAdmission;
                DirectServerSessionEffect::Accepted(accepted)
            }
            (
                DirectServerSessionPhase::CheckingRemovedPeer(_),
                DirectServerSessionInput::RemovedPeerResult(Ok(true)),
            ) => DirectServerSessionEffect::Close(DirectServerSessionCloseReason::RemovedPeer),
            (
                DirectServerSessionPhase::CheckingRemovedPeer(_),
                DirectServerSessionInput::RemovedPeerResult(Err(_)),
            ) => DirectServerSessionEffect::Close(
                DirectServerSessionCloseReason::RemovedPeerLookupFailed,
            ),
            (
                DirectServerSessionPhase::CheckingRemovedPeer(_),
                DirectServerSessionInput::Message(_),
            ) => DirectServerSessionEffect::Close(DirectServerSessionCloseReason::RepeatedHello),
            (
                DirectServerSessionPhase::AwaitingHello,
                DirectServerSessionInput::RemovedPeerResult(_),
            ) => DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput),
            (
                phase @ DirectServerSessionPhase::AwaitingRegistryAdmission,
                DirectServerSessionInput::Message(message),
            ) if matches!(message.as_ref(), LanSyncMessage::Hello { .. }) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::RepeatedHello)
            }
            (
                phase @ DirectServerSessionPhase::AwaitingRegistryAdmission,
                DirectServerSessionInput::Message(message),
            ) => {
                self.phase = phase;
                if requires_registry_admission(message.as_ref()) {
                    DirectServerSessionEffect::Ignore
                } else {
                    DirectServerSessionEffect::Dispatch(Box::new(classify_server_message(*message)))
                }
            }
            (
                phase @ DirectServerSessionPhase::AwaitingRegistryAdmission,
                DirectServerSessionInput::RemovedPeerResult(_),
            ) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
            }
            (
                DirectServerSessionPhase::AwaitingPolicy,
                DirectServerSessionInput::Message(message),
            ) if matches!(message.as_ref(), LanSyncMessage::SyncPolicy { .. }) => {
                self.phase = DirectServerSessionPhase::PersistingPolicy;
                DirectServerSessionEffect::Dispatch(Box::new(classify_server_message(*message)))
            }
            (
                phase @ DirectServerSessionPhase::AwaitingPolicy,
                DirectServerSessionInput::Message(message),
            ) => {
                self.phase = phase;
                if requires_registry_admission(message.as_ref()) {
                    DirectServerSessionEffect::Ignore
                } else {
                    DirectServerSessionEffect::Dispatch(Box::new(classify_server_message(*message)))
                }
            }
            (phase @ DirectServerSessionPhase::AwaitingPolicy, _) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
            }
            (phase @ DirectServerSessionPhase::PersistingPolicy, _) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
            }
            (
                phase @ DirectServerSessionPhase::Ready,
                DirectServerSessionInput::Message(message),
            ) if matches!(message.as_ref(), LanSyncMessage::Hello { .. }) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::RepeatedHello)
            }
            (
                phase @ DirectServerSessionPhase::Ready,
                DirectServerSessionInput::Message(message),
            ) if matches!(message.as_ref(), LanSyncMessage::SyncPolicy { .. }) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
            }
            (
                phase @ DirectServerSessionPhase::Ready,
                DirectServerSessionInput::Message(message),
            ) => {
                self.phase = phase;
                DirectServerSessionEffect::Dispatch(Box::new(classify_server_message(*message)))
            }
            (phase @ DirectServerSessionPhase::Ready, _) => {
                self.phase = phase;
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
            }
            (DirectServerSessionPhase::Closed, _) => {
                DirectServerSessionEffect::Close(DirectServerSessionCloseReason::AlreadyClosed)
            }
        };
        DirectServerSessionTransition { effect }
    }
}
