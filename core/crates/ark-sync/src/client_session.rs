use ark_application::{
    replication_storage::PeerIdentityBackend,
    sync_storage::{CommittedFeedPort, PeerProgressPort},
};
use ark_domain::types::VersionVector;

use crate::{
    protocol::{
        compute_hello_auth_hmac, compute_negotiation_offer_hmac, generate_auth_nonce,
        LanSyncMessage, NegotiatedProtocol, ProtocolCapability, ProtocolOffer, SyncPolicyRuleWire,
        PROTOCOL_VERSION,
    },
    registry_admission::AdmissionUpdate,
};

pub struct ConnectedClientTransport {
    pub io: crate::session_io::SessionIo,
    pub selected_address: String,
}

#[async_trait::async_trait]
pub trait DirectClientConnector: Send + Sync {
    /// The future must own all dial attempts so dropping it cancels them.
    async fn connect(
        &self,
        addresses: &[String],
    ) -> Result<Option<ConnectedClientTransport>, crate::session_io::SessionIoError>;
}

mod direct;
mod hello;
pub use direct::{
    DirectClientMessage, DirectClientSession, DirectClientSessionEffect, DirectClientSessionInput,
    DirectClientSessionRejection,
};
pub use hello::{
    accept_server_hello, AcceptedServerHello, ServerHelloRejection, ServerHelloValidationRequest,
};

pub struct ClientHello {
    pub message: LanSyncMessage,
    pub offer: ProtocolOffer,
    pub auth_nonce: Option<String>,
}

pub struct ClientHelloRequest<'a> {
    pub device_id: &'a str,
    pub device_name: &'a str,
    pub space_id: &'a str,
    pub addresses: &'a [String],
    pub auth_secret: Option<&'a str>,
    pub task_merge_bootstrap_v2: bool,
    pub platform: Option<String>,
    pub app_version: Option<String>,
}

pub fn build_client_hello(request: ClientHelloRequest<'_>) -> ClientHello {
    let ClientHelloRequest {
        device_id,
        device_name,
        space_id,
        addresses,
        auth_secret,
        task_merge_bootstrap_v2,
        platform,
        app_version,
    } = request;
    let (auth_nonce, auth_hmac) = match auth_secret {
        Some(secret) => {
            let nonce = generate_auth_nonce();
            let hmac = compute_hello_auth_hmac(secret, space_id, device_id, &nonce);
            (Some(nonce), Some(hmac))
        }
        None => (None, None),
    };
    let mut offer = ProtocolOffer::current_with_registry_history(true);
    if task_merge_bootstrap_v2 {
        offer = offer.with_task_merge_bootstrap_v2();
    }
    let negotiation_hmac = auth_secret
        .zip(auth_nonce.as_deref())
        .map(|(secret, nonce)| {
            compute_negotiation_offer_hmac(secret, space_id, device_id, nonce, &offer)
        });
    let hello = LanSyncMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: device_id.to_owned(),
        device_name: device_name.to_owned(),
        space_id: space_id.to_owned(),
        addresses: Some(addresses.to_owned()),
        auth_nonce: auth_nonce.clone(),
        auth_hmac,
        platform,
        app_version,
        supported_protocol_versions: Some(offer.version_ids()),
        supported_capabilities: Some(offer.capability_ids()),
        negotiated_capabilities: None,
        negotiation_hmac,
    };
    ClientHello {
        message: hello,
        offer,
        auth_nonce,
    }
}

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

#[cfg(test)]
mod hello_tests {
    use super::*;

    #[test]
    fn client_hello_preserves_metadata_and_authenticated_offer() {
        let addresses = vec!["127.0.0.1:1234".to_owned()];
        let hello = build_client_hello(ClientHelloRequest {
            device_id: "device-a",
            device_name: "Device A",
            space_id: "space-a",
            addresses: &addresses,
            auth_secret: Some("secret"),
            task_merge_bootstrap_v2: true,
            platform: Some("linux".to_owned()),
            app_version: Some("1.2.3".to_owned()),
        });
        let LanSyncMessage::Hello {
            protocol_version,
            device_id,
            device_name,
            space_id,
            addresses: advertised_addresses,
            auth_nonce: Some(nonce),
            auth_hmac: Some(auth_hmac),
            platform,
            app_version,
            supported_protocol_versions: Some(versions),
            supported_capabilities: Some(capabilities),
            negotiated_capabilities,
            negotiation_hmac: Some(negotiation_hmac),
        } = hello.message
        else {
            panic!("builder must return a complete Hello");
        };

        assert_eq!(protocol_version, PROTOCOL_VERSION);
        assert_eq!(device_id, "device-a");
        assert_eq!(device_name, "Device A");
        assert_eq!(space_id, "space-a");
        assert_eq!(advertised_addresses, Some(addresses));
        assert_eq!(platform.as_deref(), Some("linux"));
        assert_eq!(app_version.as_deref(), Some("1.2.3"));
        assert_eq!(negotiated_capabilities, None);
        assert_eq!(
            versions,
            hello.offer.version_ids(),
            "serialized offer must match the offer retained for selection validation"
        );
        assert_eq!(capabilities, hello.offer.capability_ids());
        assert_eq!(
            auth_hmac,
            compute_hello_auth_hmac("secret", "space-a", "device-a", &nonce)
        );
        assert_eq!(
            negotiation_hmac,
            compute_negotiation_offer_hmac("secret", "space-a", "device-a", &nonce, &hello.offer)
        );
        assert_eq!(hello.auth_nonce.as_deref(), Some(nonce.as_str()));
    }
}
