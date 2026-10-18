#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn accepted_client_hello_keeps_peer_metadata_and_builds_negotiated_reply() {
    let peer_offer =
        ProtocolOffer::current_with_registry_history(true).with_task_merge_bootstrap_v2();
    let request = responder_request(None);
    let accepted = match accept_client_hello(client_hello("space-a", &peer_offer), request) {
        Ok(accepted) => accepted,
        Err(_) => panic!("valid client Hello should be accepted"),
    };

    assert_eq!(accepted.peer_device_id, "client-device");
    assert_eq!(accepted.peer_device_name, "Client");
    assert_eq!(accepted.peer_addresses, ["127.0.0.1:9000"]);
    assert_eq!(accepted.peer_platform.as_deref(), Some("linux"));
    assert_eq!(accepted.peer_app_version.as_deref(), Some("1.2.3"));
    assert!(accepted
        .protocol
        .capabilities
        .contains(&ProtocolCapability::RegistryHistoryV1));

    let LanSyncMessage::Hello {
        protocol_version,
        device_id,
        device_name,
        space_id,
        addresses,
        platform,
        app_version,
        supported_protocol_versions,
        supported_capabilities,
        negotiated_capabilities,
        ..
    } = accepted.reply
    else {
        panic!("server session should return a Hello reply");
    };
    assert_eq!(protocol_version, accepted.protocol.version);
    assert_eq!(device_id, "server-device");
    assert_eq!(device_name, "Server");
    assert_eq!(space_id, "space-a");
    assert_eq!(addresses, Some(vec!["127.0.0.1:9001".to_owned()]));
    assert_eq!(platform.as_deref(), Some("windows"));
    assert_eq!(app_version.as_deref(), Some("2.3.4"));
    assert!(supported_protocol_versions
        .as_ref()
        .is_some_and(|versions| versions.contains(&accepted.protocol.version)));
    assert!(supported_capabilities
        .as_ref()
        .is_some_and(|capabilities| capabilities.contains(&"task_merge_bootstrap_v2".to_owned())));
    assert_eq!(
        negotiated_capabilities,
        Some(vec!["registry_history_v1".to_owned()])
    );
}

#[test]
fn client_hello_rejects_space_authentication_and_registry_history_mismatches() {
    let peer_offer = ProtocolOffer::current_with_registry_history(true);
    assert!(matches!(
        accept_client_hello(
            client_hello("other-space", &peer_offer),
            responder_request(None)
        ),
        Err(ClientHelloRejection::DifferentSpace)
    ));

    assert!(matches!(
        accept_client_hello(
            client_hello("space-a", &peer_offer),
            responder_request(Some("secret"))
        ),
        Err(ClientHelloRejection::InvalidAuthentication)
    ));

    let registryless_offer = ProtocolOffer::current(true);
    assert!(matches!(
        accept_client_hello(
            client_hello("space-a", &registryless_offer),
            responder_request(None)
        ),
        Err(ClientHelloRejection::Protocol {
            error: ProtocolNegotiationError::MissingRegistryHistory,
            ..
        })
    ));
}

fn responder_request(auth_secret: Option<&str>) -> ResponderHelloRequest {
    ResponderHelloRequest {
        device_id: "server-device".to_owned(),
        device_name: "Server".to_owned(),
        space_id: "space-a".to_owned(),
        addresses: vec!["127.0.0.1:9001".to_owned()],
        auth_secret: auth_secret.map(str::to_owned),
        task_merge_bootstrap_v2: true,
        platform: Some("windows".to_owned()),
        app_version: Some("2.3.4".to_owned()),
    }
}

fn client_hello(space_id: &str, offer: &ProtocolOffer) -> LanSyncMessage {
    LanSyncMessage::Hello {
        protocol_version: offer.preferred_version,
        device_id: "client-device".to_owned(),
        device_name: "Client".to_owned(),
        space_id: space_id.to_owned(),
        addresses: Some(vec!["127.0.0.1:9000".to_owned()]),
        auth_nonce: None,
        auth_hmac: None,
        platform: Some("linux".to_owned()),
        app_version: Some("1.2.3".to_owned()),
        supported_protocol_versions: Some(offer.version_ids()),
        supported_capabilities: Some(offer.capability_ids()),
        negotiated_capabilities: None,
        negotiation_hmac: None,
    }
}
