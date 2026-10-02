#[test]
fn expired_lease_gets_a_monotonic_fence_and_refresh_cas() {
    let old = acquire_refresh_lease(None, "integration-a", "node-a", 2, 10, 5).unwrap();
    assert!(acquire_refresh_lease(Some(&old), "integration-a", "node-b", 2, 12, 5).is_err());
    let next = acquire_refresh_lease(Some(&old), "integration-a", "node-b", 3, 15, 5).unwrap();
    assert_eq!(next.fencing_token, old.fencing_token + 1);
    let mut next_envelope = envelope();
    next_envelope.credential_generation = 3;
    next_envelope.refresh_fencing_token = next.fencing_token;
    next_envelope.issuer_node_id = "node-b".into();
    next_envelope.hlc = "2026-08-30T00:00:00.000Z:000004:node-b".into();
    next_envelope.key_id = encryption_key_id("enc-b");
    let next_node = AuthorizedNode {
        node_id: "node-b".into(),
        key_fingerprint: "fp-b".into(),
        signing_public_key: "sign-b".into(),
        encryption_public_key: "enc-b".into(),
        transport_public_key: None,
        grant_epoch: 2,
        status: NodeStatus::Active,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-b".into(),
    };
    let next_grant = IntegrationNodeGrant {
        node_id: "node-b".into(),
        node_encryption_key: "enc-b".into(),
        ..grant(GrantStatus::Active)
    };
    next_envelope.recipient_node_id = "node-b".into();
    next_grant.validate().unwrap();
    let mut stale_key = next_envelope.clone();
    stale_key.key_id = encryption_key_id("enc-a");
    assert!(matches!(
        validate_refresh_publication(&stale_key, &next, &next_node, &next_grant, 2, 19),
        Err(IntegrationContractError::Mismatch { field: "key_id" })
    ));
    assert_eq!(
        validate_refresh_publication(&next_envelope, &next, &next_node, &next_grant, 2, 19)
            .unwrap(),
        3
    );
    assert!(matches!(
        validate_refresh_publication(&next_envelope, &next, &next_node, &next_grant, 3, 19),
        Err(IntegrationContractError::NonMonotonic {
            field: "credential_generation"
        })
    ));
}
