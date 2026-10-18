use crate::protocol::{
    validate_protocol_selection, verify_hello_auth_hmac, LanSyncMessage, NegotiatedProtocol,
    ProtocolCapability, ProtocolNegotiationError, ProtocolOffer,
};

#[derive(Clone, Copy)]
pub struct ServerHelloValidationRequest<'a> {
    pub local_device_id: &'a str,
    pub local_space_id: &'a str,
    pub auth_secret: Option<&'a str>,
    pub local_offer: &'a ProtocolOffer,
    pub local_auth_nonce: Option<&'a str>,
}

#[derive(Debug)]
pub struct AcceptedServerHello {
    pub device_id: String,
    pub device_name: String,
    pub platform: Option<String>,
    pub app_version: Option<String>,
    pub protocol: NegotiatedProtocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerHelloRejection {
    UnexpectedMessage,
    DifferentSpace,
    InvalidAuthentication,
    InvalidProtocolSelection(ProtocolNegotiationError),
    MissingRegistryHistory,
}

impl ServerHelloRejection {
    pub fn log_message(self) -> String {
        match self {
            Self::UnexpectedMessage => "expected server hello message".to_owned(),
            Self::DifferentSpace => "Rejecting peer hello from a different space".to_owned(),
            Self::InvalidAuthentication => "Rejecting peer hello with invalid HMAC".to_owned(),
            Self::InvalidProtocolSelection(error) => {
                format!("invalid protocol selection: {}", error.code())
            }
            Self::MissingRegistryHistory => "peer requires registry-history upgrade".to_owned(),
        }
    }
}

impl std::fmt::Display for ServerHelloRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.log_message())
    }
}

pub fn accept_server_hello(
    message: LanSyncMessage,
    request: ServerHelloValidationRequest<'_>,
) -> Result<AcceptedServerHello, ServerHelloRejection> {
    let LanSyncMessage::Hello {
        protocol_version,
        device_id,
        device_name,
        space_id,
        auth_nonce,
        auth_hmac,
        platform,
        app_version,
        supported_protocol_versions,
        supported_capabilities,
        negotiated_capabilities,
        negotiation_hmac,
        ..
    } = message
    else {
        return Err(ServerHelloRejection::UnexpectedMessage);
    };

    if space_id != request.local_space_id {
        return Err(ServerHelloRejection::DifferentSpace);
    }
    if let Some(secret) = request.auth_secret {
        let valid = match (auth_nonce.as_deref(), auth_hmac.as_deref()) {
            (Some(nonce), Some(hmac)) => {
                verify_hello_auth_hmac(secret, &space_id, &device_id, nonce, hmac)
            }
            _ => false,
        };
        if !valid {
            return Err(ServerHelloRejection::InvalidAuthentication);
        }
    }

    let protocol = validate_protocol_selection(
        request.local_offer,
        protocol_version,
        supported_protocol_versions.as_deref(),
        supported_capabilities.as_deref(),
        negotiated_capabilities.as_deref(),
        request.auth_secret,
        &space_id,
        request.local_device_id,
        &device_id,
        request.local_auth_nonce,
        auth_nonce.as_deref(),
        negotiation_hmac.as_deref(),
    )
    .map_err(ServerHelloRejection::InvalidProtocolSelection)?;
    if !protocol
        .capabilities
        .contains(&ProtocolCapability::RegistryHistoryV1)
    {
        return Err(ServerHelloRejection::MissingRegistryHistory);
    }

    Ok(AcceptedServerHello {
        device_id,
        device_name,
        platform,
        app_version,
        protocol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_server_hello_keeps_identity_metadata_and_negotiated_protocol() {
        let local_offer = ProtocolOffer::current_with_registry_history(true);
        let remote_offer = ProtocolOffer::current_with_registry_history(true);
        let protocol =
            crate::protocol::negotiate_protocol(&local_offer, &remote_offer, false).unwrap();
        let message = LanSyncMessage::Hello {
            protocol_version: protocol.version,
            device_id: "server-device".to_owned(),
            device_name: "Server".to_owned(),
            space_id: "space-a".to_owned(),
            addresses: None,
            auth_nonce: None,
            auth_hmac: None,
            platform: Some("linux".to_owned()),
            app_version: Some("1.2.3".to_owned()),
            supported_protocol_versions: Some(remote_offer.version_ids()),
            supported_capabilities: Some(remote_offer.capability_ids()),
            negotiated_capabilities: Some(capability_ids(&protocol)),
            negotiation_hmac: None,
        };

        let accepted = accept_server_hello(
            message,
            ServerHelloValidationRequest {
                local_device_id: "client-device",
                local_space_id: "space-a",
                auth_secret: None,
                local_offer: &local_offer,
                local_auth_nonce: None,
            },
        )
        .unwrap();

        assert_eq!(accepted.device_id, "server-device");
        assert_eq!(accepted.device_name, "Server");
        assert_eq!(accepted.platform.as_deref(), Some("linux"));
        assert_eq!(accepted.app_version.as_deref(), Some("1.2.3"));
        assert_eq!(accepted.protocol, protocol);
    }

    #[test]
    fn server_hello_requires_matching_space_authentication_and_registry_history() {
        let local_offer = ProtocolOffer::current_with_registry_history(true);
        let remote_offer = ProtocolOffer::current(true);
        let protocol =
            crate::protocol::negotiate_protocol(&local_offer, &remote_offer, false).unwrap();
        let request = ServerHelloValidationRequest {
            local_device_id: "client-device",
            local_space_id: "space-a",
            auth_secret: Some("secret"),
            local_offer: &local_offer,
            local_auth_nonce: Some("client-nonce"),
        };

        assert_eq!(
            accept_server_hello(
                server_hello("other-space", &remote_offer, &protocol),
                request,
            )
            .unwrap_err(),
            ServerHelloRejection::DifferentSpace
        );
        assert_eq!(
            accept_server_hello(server_hello("space-a", &remote_offer, &protocol), request,)
                .unwrap_err(),
            ServerHelloRejection::InvalidAuthentication
        );

        let registryless_offer = ProtocolOffer::current(false);
        let registryless_protocol =
            crate::protocol::negotiate_protocol(&local_offer, &registryless_offer, false).unwrap();
        let registryless = server_hello("space-a", &registryless_offer, &registryless_protocol);
        let request_without_auth = ServerHelloValidationRequest {
            auth_secret: None,
            local_auth_nonce: None,
            ..request
        };
        assert_eq!(
            accept_server_hello(registryless, request_without_auth).unwrap_err(),
            ServerHelloRejection::MissingRegistryHistory
        );
    }

    fn server_hello(
        space_id: &str,
        offer: &ProtocolOffer,
        protocol: &NegotiatedProtocol,
    ) -> LanSyncMessage {
        LanSyncMessage::Hello {
            protocol_version: protocol.version,
            device_id: "server-device".to_owned(),
            device_name: "Server".to_owned(),
            space_id: space_id.to_owned(),
            addresses: None,
            auth_nonce: None,
            auth_hmac: None,
            platform: None,
            app_version: None,
            supported_protocol_versions: Some(offer.version_ids()),
            supported_capabilities: Some(offer.capability_ids()),
            negotiated_capabilities: Some(capability_ids(protocol)),
            negotiation_hmac: None,
        }
    }

    fn capability_ids(protocol: &NegotiatedProtocol) -> Vec<String> {
        protocol
            .capabilities
            .iter()
            .map(|capability| capability.wire_id().to_owned())
            .collect()
    }
}
