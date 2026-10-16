use super::*;

fn offer(versions: Option<&[u32]>, capabilities: &[&str]) -> ProtocolOffer {
    let capabilities = capabilities
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    ProtocolOffer::new(1, versions, Some(&capabilities)).unwrap()
}

#[test]
fn legacy_offer_means_only_its_current_version_and_no_capabilities() {
    let legacy = ProtocolOffer::new(1, None, None).unwrap();
    let modern = offer(Some(&[1]), &[DURABLE_RECEIPT_ACK_V1]);

    let session = negotiate_protocol(&modern, &legacy, true).unwrap();

    assert_eq!(session.version, 1);
    assert!(session.capabilities.is_empty());
}

#[test]
fn only_authenticated_intersection_enables_known_capabilities() {
    let left = offer(Some(&[1]), &[DURABLE_RECEIPT_ACK_V1, "future.cap"]);
    let right = offer(Some(&[1]), &[DURABLE_RECEIPT_ACK_V1, "unknown.cap"]);

    assert_eq!(
        negotiate_protocol(&left, &right, false)
            .unwrap()
            .capabilities,
        BTreeSet::new()
    );
    assert_eq!(
        negotiate_protocol(&left, &right, true)
            .unwrap()
            .capabilities,
        BTreeSet::from([ProtocolCapability::DurableReceiptAckV1])
    );
}

#[test]
fn registry_history_is_required_wire_support_without_claiming_peer_identity() {
    let local = ProtocolOffer::current_with_registry_history(true);
    let remote = offer(None, &["registry_history_v1"]);
    let selected = negotiate_protocol(&local, &remote, false).unwrap();
    assert!(selected
        .capabilities
        .contains(&ProtocolCapability::RegistryHistoryV1));
    assert!(!selected
        .capabilities
        .contains(&ProtocolCapability::DurableReceiptAckV1));

    let legacy = offer(None, &[]);
    let selected = negotiate_protocol(&local, &legacy, false).unwrap();
    assert!(!selected
        .capabilities
        .contains(&ProtocolCapability::RegistryHistoryV1));
}

#[test]
fn invalid_and_disjoint_offers_fail_deterministically() {
    assert_eq!(
        ProtocolOffer::new(1, Some(&[]), None),
        Err(ProtocolNegotiationError::InvalidVersionOffer)
    );
    assert_eq!(
        ProtocolOffer::new(1, Some(&[1, 1]), None),
        Err(ProtocolNegotiationError::InvalidVersionOffer)
    );
    assert_eq!(
        ProtocolOffer::new(1, Some(&[2]), None),
        Err(ProtocolNegotiationError::InvalidVersionOffer)
    );
    assert_eq!(
        ProtocolOffer::new(1, None, Some(&["bad capability".into()])),
        Err(ProtocolNegotiationError::InvalidCapabilityOffer)
    );

    let local = offer(Some(&[1]), &[]);
    let remote = ProtocolOffer::new(2, Some(&[2]), None).unwrap();
    assert_eq!(
        negotiate_protocol(&local, &remote, true),
        Err(ProtocolNegotiationError::NoCommonVersion)
    );
}

#[test]
fn negotiation_hmac_binds_offers_and_selection() {
    let initiator = offer(Some(&[1]), &[DURABLE_RECEIPT_ACK_V1]);
    let responder = offer(Some(&[1]), &[DURABLE_RECEIPT_ACK_V1]);
    let selected = negotiate_protocol(&initiator, &responder, true).unwrap();
    let transcript = ProtocolSelectionTranscript {
        space_id: "space",
        initiator_device_id: "device-a",
        responder_device_id: "device-b",
        initiator_nonce: "nonce-a",
        responder_nonce: "nonce-b",
        initiator_offer: &initiator,
        responder_offer: &responder,
        selection: &selected,
    };
    let proof = compute_negotiation_selection_hmac("secret", &transcript);
    assert_eq!(proof.len(), 64);
    assert_eq!(
        proof,
        compute_negotiation_selection_hmac("secret", &transcript)
    );

    let changed = ProtocolOffer::new(1, Some(&[1]), None).unwrap();
    let changed_transcript = ProtocolSelectionTranscript {
        responder_offer: &changed,
        ..transcript
    };
    assert_ne!(
        proof,
        compute_negotiation_selection_hmac("secret", &changed_transcript)
    );
}

#[test]
fn responder_selection_requires_matching_offers_and_proof() {
    let initiator = ProtocolOffer::current(true);
    let responder = ProtocolOffer::current(true);
    let selected = negotiate_protocol(&initiator, &responder, true).unwrap();
    let selected_ids = selected
        .capabilities
        .iter()
        .map(|capability| capability.wire_id().to_owned())
        .collect::<Vec<_>>();
    let transcript = ProtocolSelectionTranscript {
        space_id: "space",
        initiator_device_id: "initiator",
        responder_device_id: "responder",
        initiator_nonce: "nonce-a",
        responder_nonce: "nonce-b",
        initiator_offer: &initiator,
        responder_offer: &responder,
        selection: &selected,
    };
    let proof = compute_negotiation_selection_hmac("secret", &transcript);

    assert_eq!(
        validate_protocol_selection(
            &initiator,
            selected.version,
            Some(&responder.version_ids()),
            Some(&responder.capability_ids()),
            Some(&selected_ids),
            Some("secret"),
            "space",
            "initiator",
            "responder",
            Some("nonce-a"),
            Some("nonce-b"),
            Some(&proof),
        ),
        Ok(selected.clone())
    );
    assert_eq!(
        validate_protocol_selection(
            &initiator,
            selected.version,
            Some(&responder.version_ids()),
            Some(&responder.capability_ids()),
            Some(&[]),
            Some("secret"),
            "space",
            "initiator",
            "responder",
            Some("nonce-a"),
            Some("nonce-b"),
            Some(&proof),
        ),
        Err(ProtocolNegotiationError::InvalidSelection)
    );
}

#[test]
fn legacy_selection_has_no_semantic_capabilities() {
    let local = ProtocolOffer::current_with_registry_history(true);
    let selected = validate_protocol_selection(
        &local,
        1,
        None,
        None,
        None,
        None,
        "space",
        "initiator",
        "responder",
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(selected.version, 1);
    assert!(selected.capabilities.is_empty());
}
