use ark_sync_contracts::wire::LanSyncMessage;

use super::{DirectClientSession, DirectClientSessionRejection, Phase};

pub enum DirectClientSessionEffect {
    Dispatch(Box<DirectClientMessage>),
    Ignore,
    Close(DirectClientSessionRejection),
}

pub enum DirectClientSessionInput {
    Message(Box<DirectClientMessage>),
    Ignore,
    Rejected(DirectClientSessionRejection),
    Malformed,
    Closed,
    Failed(String),
}

impl DirectClientSessionEffect {
    fn dispatch(message: DirectClientMessage) -> Self {
        Self::Dispatch(Box::new(message))
    }
}

pub enum DirectClientMessage {
    Hello(LanSyncMessage),
    RegistryControl(LanSyncMessage),
    SyncPolicy(crate::client_session::SyncPolicyUpdate),
    VersionVector(ark_domain::types::VersionVector),
    SyncChanges {
        batch_id: String,
        entities: Vec<ark_domain::types::SyncEntity>,
        is_last: bool,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
        usage_complete_through: Option<std::collections::HashMap<String, u64>>,
    },
    SyncAck {
        batch_id: String,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    },
    SignedIntegrationFrame(ark_domain::integration_replication::SignedSyncEnvelope),
    SignedIntegrationAck,
    LiveChange {
        change_id: String,
        entity: ark_domain::types::SyncEntity,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    },
    LiveAck {
        change_id: String,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    },
    LiveReceiptAck {
        change_id: String,
        policy_identity: Option<ark_domain::sync_policy::PolicyIdentity>,
    },
    TaskMergeBatch(ark_domain::task_merge_sync::TaskMergeBatch),
    TaskMergeBatchAck,
    Ping {
        ts: u64,
    },
    Pong,
    PairingRejected,
    PairingAccepted,
    ProtocolRejected {
        origin_device_id: String,
        target_device_id: String,
        reason: String,
    },
    PeerList(Vec<ark_domain::types::PeerRecord>),
}

impl DirectClientSession {
    pub fn transition(&self, message: LanSyncMessage) -> DirectClientSessionEffect {
        if matches!(message, LanSyncMessage::Hello { .. }) {
            return if self.phase == Phase::AwaitingHello {
                DirectClientSessionEffect::dispatch(DirectClientMessage::Hello(message))
            } else {
                DirectClientSessionEffect::Close(DirectClientSessionRejection::RepeatedHello)
            };
        }

        if is_registry_control(&message) {
            return if self.phase == Phase::AwaitingHello {
                DirectClientSessionEffect::Close(
                    DirectClientSessionRejection::RegistryControlBeforeHello,
                )
            } else {
                DirectClientSessionEffect::dispatch(DirectClientMessage::RegistryControl(message))
            };
        }

        if !self.is_ready() && ark_sync_contracts::wire::requires_registry_admission(&message) {
            return DirectClientSessionEffect::Ignore;
        }

        let event = match message {
            LanSyncMessage::SyncPolicy {
                revision,
                digest,
                active,
                rules,
            } => DirectClientMessage::SyncPolicy(crate::client_session::SyncPolicyUpdate {
                revision,
                digest,
                active,
                rules,
            }),
            LanSyncMessage::VersionVector { vector, .. } => {
                DirectClientMessage::VersionVector(vector)
            }
            LanSyncMessage::SyncChanges {
                batch_id,
                entities,
                is_last,
                policy_identity,
                usage_complete_through,
                ..
            } => DirectClientMessage::SyncChanges {
                batch_id,
                entities,
                is_last,
                policy_identity,
                usage_complete_through,
            },
            LanSyncMessage::SyncAck {
                batch_id,
                policy_identity,
                ..
            } => DirectClientMessage::SyncAck {
                batch_id,
                policy_identity,
            },
            LanSyncMessage::SignedIntegrationFrame { frame } => {
                DirectClientMessage::SignedIntegrationFrame(frame)
            }
            LanSyncMessage::SignedIntegrationAck { .. } => {
                DirectClientMessage::SignedIntegrationAck
            }
            LanSyncMessage::LiveChange {
                change_id,
                entity,
                policy_identity,
                ..
            } => DirectClientMessage::LiveChange {
                change_id,
                entity,
                policy_identity,
            },
            LanSyncMessage::LiveAck {
                change_id,
                policy_identity,
            } => DirectClientMessage::LiveAck {
                change_id,
                policy_identity,
            },
            LanSyncMessage::LiveReceiptAck {
                change_id,
                policy_identity,
            } => DirectClientMessage::LiveReceiptAck {
                change_id,
                policy_identity,
            },
            LanSyncMessage::TaskMergeBatch { batch } => DirectClientMessage::TaskMergeBatch(batch),
            LanSyncMessage::TaskMergeBatchAck { .. } => DirectClientMessage::TaskMergeBatchAck,
            LanSyncMessage::Ping { ts } => DirectClientMessage::Ping { ts },
            LanSyncMessage::Pong { .. } => DirectClientMessage::Pong,
            LanSyncMessage::PairingRejected { .. } => DirectClientMessage::PairingRejected,
            LanSyncMessage::PairingAccepted { .. } => DirectClientMessage::PairingAccepted,
            LanSyncMessage::ProtocolRejected {
                origin_device_id,
                target_device_id,
                reason,
                ..
            } => DirectClientMessage::ProtocolRejected {
                origin_device_id,
                target_device_id,
                reason,
            },
            LanSyncMessage::PeerList { peers } => DirectClientMessage::PeerList(peers),
            LanSyncMessage::Hello { .. } => {
                return DirectClientSessionEffect::Close(
                    DirectClientSessionRejection::RepeatedHello,
                );
            }
            message @ (LanSyncMessage::RegistryAdmissionStart { .. }
            | LanSyncMessage::RegistryAdmissionProof { .. }
            | LanSyncMessage::RegistryAdmissionConfirm { .. }
            | LanSyncMessage::RegistrySyncStart { .. }
            | LanSyncMessage::RegistryEventsPage { .. }
            | LanSyncMessage::RegistrySyncReady { .. }
            | LanSyncMessage::RegistryEventUpdate { .. }) => {
                return DirectClientSessionEffect::dispatch(DirectClientMessage::RegistryControl(
                    message,
                ));
            }
        };
        DirectClientSessionEffect::dispatch(event)
    }
}

fn is_registry_control(message: &LanSyncMessage) -> bool {
    matches!(
        message,
        LanSyncMessage::RegistryAdmissionStart { .. }
            | LanSyncMessage::RegistryAdmissionProof { .. }
            | LanSyncMessage::RegistryAdmissionConfirm { .. }
            | LanSyncMessage::RegistrySyncStart { .. }
            | LanSyncMessage::RegistryEventsPage { .. }
            | LanSyncMessage::RegistrySyncReady { .. }
            | LanSyncMessage::RegistryEventUpdate { .. }
    )
}
