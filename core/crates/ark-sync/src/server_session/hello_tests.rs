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

#[test]
fn direct_server_session_waits_for_block_check_and_accepts_only_once() {
    let offer = ProtocolOffer::current_with_registry_history(true);
    let mut session = DirectServerSession::new(responder_request(None));

    let first = session.transition(DirectServerSessionInput::Message(Box::new(client_hello(
        "space-a", &offer,
    ))));
    assert!(matches!(
        &first.effect,
        DirectServerSessionEffect::CheckRemovedPeer { device_id }
            if device_id == "client-device"
    ));

    let repeated = session.transition(DirectServerSessionInput::Message(Box::new(client_hello(
        "space-a", &offer,
    ))));
    assert!(matches!(
        &repeated.effect,
        DirectServerSessionEffect::Close(DirectServerSessionCloseReason::RepeatedHello)
    ));
}

#[test]
fn direct_server_session_accepts_after_clear_block_check_and_fails_closed() {
    let offer = ProtocolOffer::current_with_registry_history(true);
    let mut allowed = DirectServerSession::new(responder_request(None));
    allowed.transition(DirectServerSessionInput::Message(Box::new(client_hello(
        "space-a", &offer,
    ))));
    let accepted = allowed.transition(DirectServerSessionInput::RemovedPeerResult(Ok(false)));
    assert!(matches!(
        &accepted.effect,
        DirectServerSessionEffect::Accepted(accepted)
            if accepted.peer_device_id == "client-device"
    ));

    for result in [Ok(true), Err("storage unavailable".to_owned())] {
        let mut blocked = DirectServerSession::new(responder_request(None));
        blocked.transition(DirectServerSessionInput::Message(Box::new(client_hello(
            "space-a", &offer,
        ))));
        let transition = blocked.transition(DirectServerSessionInput::RemovedPeerResult(result));
        assert!(matches!(
            &transition.effect,
            DirectServerSessionEffect::Close(
                DirectServerSessionCloseReason::RemovedPeer
                    | DirectServerSessionCloseReason::RemovedPeerLookupFailed
            )
        ));
    }
}

#[test]
fn direct_server_session_rejects_self_and_protocol_mismatches() {
    let offer = ProtocolOffer::current_with_registry_history(true);
    let mut self_hello = client_hello("space-a", &offer);
    let LanSyncMessage::Hello { device_id, .. } = &mut self_hello else {
        panic!("test helper builds a Hello message");
    };
    *device_id = "server-device".to_owned();
    let mut session = DirectServerSession::new(responder_request(None));
    let self_result = session.transition(DirectServerSessionInput::Message(Box::new(self_hello)));
    assert!(matches!(
        &self_result.effect,
        DirectServerSessionEffect::Close(DirectServerSessionCloseReason::SelfConnection)
    ));

    let mut wrong_space = DirectServerSession::new(responder_request(None));
    let mismatch = wrong_space.transition(DirectServerSessionInput::Message(Box::new(
        client_hello("other-space", &offer),
    )));
    assert!(matches!(
        &mismatch.effect,
        DirectServerSessionEffect::Rejected(ClientHelloRejection::DifferentSpace)
    ));

    let mut invalid_auth = DirectServerSession::new(responder_request(Some("secret")));
    let invalid = invalid_auth.transition(DirectServerSessionInput::Message(Box::new(
        client_hello("space-a", &offer),
    )));
    assert!(matches!(
        &invalid.effect,
        DirectServerSessionEffect::Rejected(ClientHelloRejection::InvalidAuthentication)
    ));
}

#[test]
fn policy_bound_sync_starts_once_only_after_session_admission() {
    let offer = ProtocolOffer::current_with_registry_history(true);
    let mut session = DirectServerSession::new(responder_request(None));
    session.transition(DirectServerSessionInput::Message(Box::new(client_hello(
        "space-a", &offer,
    ))));
    session.transition(DirectServerSessionInput::RemovedPeerResult(Ok(false)));

    assert!(!session.start_sync_after_policy_persisted());
    assert!(session.mark_ready(DirectServerSyncStartPolicy::AfterPolicyPersistence));
    assert!(!session.start_sync_after_policy_persisted());
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(
                crate::protocol::default_sync_policy_message()
            )))
            .effect,
        DirectServerSessionEffect::Dispatch(message)
            if matches!(message.as_ref(), DirectServerMessage::SyncPolicy(_))
    ));
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(
                crate::protocol::default_sync_policy_message()
            )))
            .effect,
        DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
    ));
    assert!(session.start_sync_after_policy_persisted());
    assert!(!session.start_sync_after_policy_persisted());
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(
                LanSyncMessage::VersionVector {
                    vector: std::collections::HashMap::new(),
                    origin_device_id: None,
                }
            )))
            .effect,
        DirectServerSessionEffect::Dispatch(message)
            if matches!(
                message.as_ref(),
                DirectServerMessage::VersionVector(vector) if vector.is_empty()
            )
    ));

    let mut legacy_session = DirectServerSession::new(responder_request(None));
    legacy_session.transition(DirectServerSessionInput::Message(Box::new(client_hello(
        "space-a", &offer,
    ))));
    legacy_session.transition(DirectServerSessionInput::RemovedPeerResult(Ok(false)));
    assert!(legacy_session.mark_ready(DirectServerSyncStartPolicy::Immediate));
    assert!(matches!(
        legacy_session
            .transition(DirectServerSessionInput::Message(Box::new(
                crate::protocol::default_sync_policy_message()
            )))
            .effect,
        DirectServerSessionEffect::Close(DirectServerSessionCloseReason::UnexpectedInput)
    ));
    assert!(!legacy_session.start_sync_after_policy_persisted());
}

#[test]
fn server_session_routes_messages_by_admission_and_policy_phase() {
    let mut session = DirectServerSession::new(responder_request(None));
    let version_vector = LanSyncMessage::VersionVector {
        vector: std::collections::HashMap::new(),
        origin_device_id: None,
    };
    let ping = LanSyncMessage::Ping { ts: 1 };

    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(
                version_vector.clone()
            )))
            .effect,
        DirectServerSessionEffect::Ignore
    ));
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(ping.clone())))
            .effect,
        DirectServerSessionEffect::Dispatch(message)
            if matches!(message.as_ref(), DirectServerMessage::Live(message) if message == &ping)
    ));

    let offer = ProtocolOffer::current_with_registry_history(true);
    session.transition(DirectServerSessionInput::Message(Box::new(client_hello(
        "space-a", &offer,
    ))));
    session.transition(DirectServerSessionInput::RemovedPeerResult(Ok(false)));
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(
                version_vector.clone()
            )))
            .effect,
        DirectServerSessionEffect::Ignore
    ));
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(ping.clone())))
            .effect,
        DirectServerSessionEffect::Dispatch(message)
            if matches!(message.as_ref(), DirectServerMessage::Live(message) if message == &ping)
    ));

    assert!(session.mark_ready(DirectServerSyncStartPolicy::AfterPolicyPersistence));
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(version_vector)))
            .effect,
        DirectServerSessionEffect::Ignore
    ));
    assert!(matches!(
        session
            .transition(DirectServerSessionInput::Message(Box::new(
                crate::protocol::default_sync_policy_message()
            )))
            .effect,
        DirectServerSessionEffect::Dispatch(message)
            if matches!(message.as_ref(), DirectServerMessage::SyncPolicy(_))
    ));
}
