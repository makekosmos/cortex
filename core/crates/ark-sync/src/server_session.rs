use ark_application::{
    registry_admission::RegistryAdmissionStorage,
    replication_storage::PeerIdentityBackend,
    sync_storage::{CommittedFeedPort, PeerProgressPort, PeerSyncPolicyPort},
};
use ark_domain::types::VersionVector;

use crate::{
    protocol::{LanSyncMessage, NegotiatedProtocol, ProtocolCapability},
    registry_admission::AdmissionUpdate,
};

pub struct DirectServerAdmission {
    admission: crate::registry_admission::DirectRegistryAdmission,
    peer_device_id: String,
    protocol: NegotiatedProtocol,
}

pub struct AdmittedServerPeer {
    peer_device_id: String,
    registry_session_id: String,
    protocol: NegotiatedProtocol,
}

pub struct AdmissionProgress {
    pub outgoing: Vec<LanSyncMessage>,
    pub applied_events: Vec<ark_domain::device_registry::RegistryEvent>,
    pub disabled_device_ids: Vec<String>,
    pub local_device_is_admitted: bool,
    pub close_session: bool,
    pub exchange_complete: bool,
    pub status: AdmissionStatus,
}

pub enum AdmissionStatus {
    Pending,
    Ready(AdmittedServerPeer),
    CloseSession,
}

impl DirectServerAdmission {
    pub fn begin(
        local_device_id: &str,
        peer_device_id: &str,
        space_id: &str,
        protocol: NegotiatedProtocol,
    ) -> Result<Self, String> {
        let admission = crate::registry_admission::DirectRegistryAdmission::responder(
            local_device_id,
            peer_device_id,
            space_id,
            protocol.clone(),
        )?;
        Ok(Self {
            admission,
            peer_device_id: peer_device_id.to_owned(),
            protocol,
        })
    }

    pub fn session_id(&self) -> Option<&str> {
        self.admission.session_id()
    }

    pub async fn handle<S: RegistryAdmissionStorage + ?Sized>(
        &mut self,
        storage: &S,
        message: LanSyncMessage,
    ) -> Result<AdmissionProgress, String> {
        let update = self.admission.handle(storage, message).await?;
        let status = if update.close_session && update.exchange_complete {
            AdmissionStatus::CloseSession
        } else if update.became_ready {
            AdmissionStatus::Ready(self.admitted_peer()?)
        } else {
            AdmissionStatus::Pending
        };
        Ok(AdmissionProgress::from_update(update, status))
    }

    pub async fn local_sync_policy_message<S: PeerIdentityBackend + ?Sized>(
        &self,
        storage: &S,
        local_device_id: &str,
    ) -> Result<Option<LanSyncMessage>, String> {
        if !self
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

    fn is_policy_bound(&self) -> bool {
        self.protocol
            .capabilities
            .contains(&ProtocolCapability::PolicyBoundDeliveryV1)
    }

    pub async fn begin_legacy_baseline<S: CommittedFeedPort + ?Sized>(
        &self,
        storage: &S,
    ) -> Result<(), String> {
        if self.is_policy_bound() {
            return Ok(());
        }
        let generation = storage
            .selective_sync_policy_generation_for_peer(&self.peer_device_id)
            .await?;
        storage
            .begin_outbound_policy(&self.peer_device_id, generation)
            .await
            .map(|_| ())
    }

    pub async fn load_initial_vector<S: PeerProgressPort + ?Sized>(
        &self,
        storage: &S,
    ) -> Result<VersionVector, String> {
        storage.load_version_vector().await
    }

    fn admitted_peer(&self) -> Result<AdmittedServerPeer, String> {
        Ok(AdmittedServerPeer {
            peer_device_id: self.peer_device_id.clone(),
            registry_session_id: self
                .admission
                .session_id()
                .ok_or_else(|| "registry_admission_session_missing".to_owned())?
                .to_owned(),
            protocol: self.protocol.clone(),
        })
    }
}

pub async fn begin_policy_bound_sync<S>(
    storage: &S,
    peer_device_id: &str,
) -> Result<VersionVector, String>
where
    S: PeerSyncPolicyPort + CommittedFeedPort + PeerProgressPort + ?Sized,
{
    let identity = storage
        .accepted_policy_identity_for_peer(peer_device_id)
        .await?
        .ok_or_else(|| "accepted sync policy identity is missing".to_owned())?;
    storage
        .begin_outbound_policy_identity(peer_device_id, &identity)
        .await?;
    storage.load_version_vector().await
}

impl AdmittedServerPeer {
    pub fn peer_device_id(&self) -> &str {
        &self.peer_device_id
    }

    pub fn registry_session_id(&self) -> &str {
        &self.registry_session_id
    }

    pub fn policy_bound_delivery(&self) -> bool {
        self.protocol
            .capabilities
            .contains(&ProtocolCapability::PolicyBoundDeliveryV1)
    }
}

impl AdmissionProgress {
    fn from_update(update: AdmissionUpdate, status: AdmissionStatus) -> Self {
        Self {
            outgoing: update.outgoing,
            applied_events: update.applied_events,
            disabled_device_ids: update.disabled_device_ids,
            local_device_is_admitted: update.local_device_is_admitted,
            close_session: update.close_session,
            exchange_complete: update.exchange_complete,
            status,
        }
    }
}
