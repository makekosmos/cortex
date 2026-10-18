use ark_application::{
    registry_admission::RegistryAdmissionStorage,
    replication_storage::PeerIdentityBackend,
    sync_storage::{CommittedFeedPort, PeerProgressPort, PeerSyncPolicyPort},
};
use ark_domain::types::VersionVector;

use crate::{
    protocol::{
        compute_hello_auth_hmac, compute_negotiation_selection_hmac, generate_auth_nonce,
        negotiate_protocol, verify_hello_auth_hmac, verify_negotiation_offer_hmac, LanSyncMessage,
        NegotiatedProtocol, ProtocolCapability, ProtocolNegotiationError, ProtocolOffer,
        ProtocolSelectionTranscript, MAX_PROTOCOL_OFFER_VERSIONS,
    },
    registry_admission::AdmissionUpdate,
};

mod direct_hello;
pub use direct_hello::{
    DirectServerHelloCloseReason, DirectServerHelloEffect, DirectServerHelloInput,
    DirectServerHelloTransition, DirectServerSession, DirectServerSyncStartPolicy,
};

#[derive(Clone)]
pub struct ResponderHelloRequest {
    pub device_id: String,
    pub device_name: String,
    pub space_id: String,
    pub addresses: Vec<String>,
    pub auth_secret: Option<String>,
    pub task_merge_bootstrap_v2: bool,
    pub platform: Option<String>,
    pub app_version: Option<String>,
}

pub struct AcceptedClientHello {
    pub peer_device_id: String,
    pub peer_device_name: String,
    pub peer_addresses: Vec<String>,
    pub peer_platform: Option<String>,
    pub peer_app_version: Option<String>,
    pub protocol: NegotiatedProtocol,
    pub reply: LanSyncMessage,
}

pub enum ClientHelloRejection {
    DifferentSpace,
    InvalidAuthentication,
    Protocol {
        peer_device_id: String,
        error: ProtocolNegotiationError,
        offered_protocol_versions: Vec<u32>,
    },
}

pub fn accept_client_hello(
    message: LanSyncMessage,
    request: ResponderHelloRequest,
) -> Result<AcceptedClientHello, ClientHelloRejection> {
    let LanSyncMessage::Hello {
        protocol_version,
        device_id: peer_device_id,
        device_name: peer_device_name,
        space_id: peer_space_id,
        addresses,
        auth_nonce,
        auth_hmac,
        platform: peer_platform,
        app_version: peer_app_version,
        supported_protocol_versions,
        supported_capabilities,
        negotiated_capabilities,
        negotiation_hmac: peer_negotiation_hmac,
    } = message
    else {
        return Err(ClientHelloRejection::Protocol {
            peer_device_id: String::new(),
            error: ProtocolNegotiationError::InvalidSelection,
            offered_protocol_versions: Vec::new(),
        });
    };

    if peer_space_id != request.space_id {
        return Err(ClientHelloRejection::DifferentSpace);
    }
    if let Some(secret) = request.auth_secret.as_deref() {
        let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
            (Some(nonce), Some(hmac)) => {
                verify_hello_auth_hmac(secret, &request.space_id, &peer_device_id, nonce, hmac)
            }
            _ => false,
        };
        if !valid {
            return Err(ClientHelloRejection::InvalidAuthentication);
        }
    }

    let mut local_offer = ProtocolOffer::current_with_registry_history(true);
    if request.task_merge_bootstrap_v2 {
        local_offer = local_offer.with_task_merge_bootstrap_v2();
    }
    if negotiated_capabilities.is_some() {
        return Err(protocol_rejection(
            peer_device_id,
            ProtocolNegotiationError::InvalidCapabilityOffer,
            Vec::new(),
        ));
    }
    let peer_offer = ProtocolOffer::new(
        protocol_version,
        supported_protocol_versions.as_deref(),
        supported_capabilities.as_deref(),
    )
    .map_err(|error| protocol_rejection(peer_device_id.clone(), error, Vec::new()))?;
    let peer_offer_authenticated = match (
        request.auth_secret.as_deref(),
        auth_nonce.as_deref(),
        peer_negotiation_hmac.as_deref(),
    ) {
        (Some(secret), Some(nonce), Some(hmac)) => verify_negotiation_offer_hmac(
            secret,
            &request.space_id,
            &peer_device_id,
            nonce,
            &peer_offer,
            hmac,
        ),
        (_, _, Some(_)) => false,
        _ => false,
    };
    if peer_negotiation_hmac.is_some() && !peer_offer_authenticated {
        return Err(protocol_rejection(
            peer_device_id,
            ProtocolNegotiationError::InvalidSelectionProof,
            peer_offer.version_ids(),
        ));
    }
    let negotiated = negotiate_protocol(&local_offer, &peer_offer, peer_offer_authenticated)
        .map_err(|error| {
            protocol_rejection(peer_device_id.clone(), error, peer_offer.version_ids())
        })?;
    if !negotiated
        .capabilities
        .contains(&ProtocolCapability::RegistryHistoryV1)
    {
        return Err(protocol_rejection(
            peer_device_id,
            ProtocolNegotiationError::MissingRegistryHistory,
            peer_offer.version_ids(),
        ));
    }

    let (reply_auth_nonce, reply_auth_hmac) = match request.auth_secret.as_deref() {
        Some(secret) => {
            let nonce = generate_auth_nonce();
            let hmac =
                compute_hello_auth_hmac(secret, &request.space_id, &request.device_id, &nonce);
            (Some(nonce), Some(hmac))
        }
        None => (None, None),
    };
    let negotiated_capabilities = negotiated
        .capabilities
        .iter()
        .map(|capability| capability.wire_id().to_owned())
        .collect();
    let reply_negotiation_hmac = if peer_offer_authenticated {
        request
            .auth_secret
            .as_deref()
            .zip(auth_nonce.as_deref())
            .zip(reply_auth_nonce.as_deref())
            .map(|((secret, peer_nonce), reply_nonce)| {
                compute_negotiation_selection_hmac(
                    secret,
                    &ProtocolSelectionTranscript {
                        space_id: &request.space_id,
                        initiator_device_id: &peer_device_id,
                        responder_device_id: &request.device_id,
                        initiator_nonce: peer_nonce,
                        responder_nonce: reply_nonce,
                        initiator_offer: &peer_offer,
                        responder_offer: &local_offer,
                        selection: &negotiated,
                    },
                )
            })
    } else {
        None
    };

    Ok(AcceptedClientHello {
        peer_device_id,
        peer_device_name,
        peer_addresses: addresses.unwrap_or_default(),
        peer_platform,
        peer_app_version,
        protocol: negotiated.clone(),
        reply: LanSyncMessage::Hello {
            protocol_version: negotiated.version,
            device_id: request.device_id,
            device_name: request.device_name,
            space_id: request.space_id,
            addresses: Some(request.addresses),
            auth_nonce: reply_auth_nonce,
            auth_hmac: reply_auth_hmac,
            platform: request.platform,
            app_version: request.app_version,
            supported_protocol_versions: Some(local_offer.version_ids()),
            supported_capabilities: Some(local_offer.capability_ids()),
            negotiated_capabilities: Some(negotiated_capabilities),
            negotiation_hmac: reply_negotiation_hmac,
        },
    })
}

fn protocol_rejection(
    peer_device_id: String,
    error: ProtocolNegotiationError,
    offered_protocol_versions: Vec<u32>,
) -> ClientHelloRejection {
    ClientHelloRejection::Protocol {
        peer_device_id,
        error,
        offered_protocol_versions: if offered_protocol_versions.len() <= MAX_PROTOCOL_OFFER_VERSIONS
        {
            offered_protocol_versions
        } else {
            Vec::new()
        },
    }
}

#[cfg(test)]
#[path = "server_session/hello_tests.rs"]
mod hello_tests;

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
