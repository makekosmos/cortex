use ark_application::{
    replication_storage::PeerIdentityBackend,
    sync_storage::{CommittedFeedPort, PeerProgressPort},
};
use ark_domain::types::VersionVector;

use crate::{
    protocol::{LanSyncMessage, NegotiatedProtocol, ProtocolCapability, SyncPolicyRuleWire},
    registry_admission::AdmissionUpdate,
};

pub struct DirectClientAdmission {
    admission: crate::registry_admission::DirectRegistryAdmission,
    peer_device_id: String,
    protocol: NegotiatedProtocol,
}

pub struct AdmittedClientPeer {
    peer_device_id: String,
    registry_session_id: String,
    protocol: NegotiatedProtocol,
}

impl AdmittedClientPeer {
    pub fn peer_device_id(&self) -> &str {
        &self.peer_device_id
    }

    pub fn registry_session_id(&self) -> &str {
        &self.registry_session_id
    }

    pub fn protocol(&self) -> &NegotiatedProtocol {
        &self.protocol
    }
}

pub struct AdmissionProgress {
    pub outgoing: Vec<LanSyncMessage>,
    pub applied_events: Vec<ark_domain::device_registry::RegistryEvent>,
    pub status: AdmissionStatus,
}

pub enum AdmissionStatus {
    Pending,
    Ready(AdmittedClientPeer),
    CloseSession,
}

impl DirectClientAdmission {
    pub fn begin(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
    ) -> Result<(Self, LanSyncMessage), String> {
        let (admission, start) = crate::registry_admission::DirectRegistryAdmission::initiator(
            local_device_id,
            peer_device_id,
            space_id,
            protocol.clone(),
        )?;
        Ok((
            Self {
                admission,
                peer_device_id: peer_device_id.to_owned(),
                protocol,
            },
            start,
        ))
    }

    pub async fn handle<
        S: ark_application::registry_admission::RegistryAdmissionStorage + ?Sized,
    >(
        &mut self,
        storage: &S,
        message: LanSyncMessage,
    ) -> Result<AdmissionProgress, String> {
        let update = self.admission.handle(storage, message).await?;
        let status = if update.close_session && update.exchange_complete {
            AdmissionStatus::CloseSession
        } else if update.became_ready {
            AdmissionStatus::Ready(Self::ready_peer(
                &self.admission,
                &self.peer_device_id,
                &self.protocol,
            )?)
        } else {
            AdmissionStatus::Pending
        };
        Ok(AdmissionProgress::from_update(update, status))
    }

    pub async fn sync_policy_message<S: PeerIdentityBackend + ?Sized>(
        &self,
        storage: &S,
        local_device_id: &str,
        ready: &AdmittedClientPeer,
    ) -> Result<Option<LanSyncMessage>, String> {
        if !ready
            .protocol
            .capabilities
            .contains(&ProtocolCapability::SelectiveSyncPolicyV1)
        {
            return Ok(None);
        }
        let profile = storage.load_local_sync_profile(local_device_id).await?;
        Ok(Some(
            profile
                .as_ref()
                .map(crate::protocol::sync_policy_message)
                .unwrap_or_else(crate::protocol::default_sync_policy_message),
        ))
    }

    pub async fn begin_initial_sync<S>(
        &self,
        storage: &S,
        ready: &AdmittedClientPeer,
    ) -> Result<Option<VersionVector>, String>
    where
        S: CommittedFeedPort + PeerProgressPort + ?Sized,
    {
        if ready
            .protocol
            .capabilities
            .contains(&ProtocolCapability::PolicyBoundDeliveryV1)
        {
            return Ok(None);
        }
        begin_initial_sync(storage, ready.peer_device_id())
            .await
            .map(Some)
    }

    fn ready_peer(
        admission: &crate::registry_admission::DirectRegistryAdmission,
        peer_device_id: &str,
        protocol: &NegotiatedProtocol,
    ) -> Result<AdmittedClientPeer, String> {
        Ok(AdmittedClientPeer {
            peer_device_id: peer_device_id.to_owned(),
            registry_session_id: admission
                .session_id()
                .ok_or_else(|| "registry_admission_session_missing".to_owned())?
                .to_owned(),
            protocol: protocol.clone(),
        })
    }
}

impl AdmissionProgress {
    fn from_update(update: AdmissionUpdate, status: AdmissionStatus) -> Self {
        Self {
            outgoing: update.outgoing,
            applied_events: update.applied_events,
            status,
        }
    }
}

pub struct SyncPolicyUpdate {
    pub revision: i64,
    pub digest: String,
    pub active: bool,
    pub rules: Vec<SyncPolicyRuleWire>,
}

pub async fn accept_sync_policy<S: PeerIdentityBackend + ?Sized>(
    storage: &S,
    peer_device_id: &str,
    protocol: &NegotiatedProtocol,
    authenticated: bool,
    update: SyncPolicyUpdate,
) -> Result<(), String> {
    if !authenticated {
        return Err("sync policy arrived before authentication".into());
    }
    if !protocol
        .capabilities
        .contains(&ProtocolCapability::SelectiveSyncPolicyV1)
    {
        return Err("sync policy capability was not negotiated".into());
    }

    let rules = crate::protocol::parse_sync_policy_rules(update.rules)?;
    storage
        .accept_peer_sync_policy(
            peer_device_id,
            update.revision,
            &update.digest,
            update.active,
            &rules,
        )
        .await
        .map(|_| ())
}

pub async fn begin_initial_sync<S>(
    storage: &S,
    peer_device_id: &str,
) -> Result<VersionVector, String>
where
    S: CommittedFeedPort + PeerProgressPort + ?Sized,
{
    let generation = storage
        .selective_sync_policy_generation_for_peer(peer_device_id)
        .await?;
    storage
        .begin_outbound_policy(peer_device_id, generation)
        .await?;
    storage
        .load_version_vector()
        .await
        .map_err(|error| format!("failed to load sync version vector: {error}"))
}
