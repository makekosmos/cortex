use super::{
    acquire_refresh_lease, load_credential_envelope, load_latest_credential_envelope,
    load_refresh_lease, load_version_vector, persist_node_authorization,
    publish_credential_envelope, IntegrationReplicationTestFixture, RefreshLeaseAcquireFixture,
};
use crate::test_support::TestDatabase;
use ark_application::integration_admin::NodeAuthorizationOperation;
use ark_domain::integration_replication::{
    encryption_key_id, AuthorizedNode, GrantStatus, IntegrationCredentialEnvelope,
    IntegrationNodeGrant, NodeStatus,
};
use std::sync::{Arc, Barrier};

#[test]
fn outbound_frame_authorization_tracks_persisted_grants() {
    let database = TestDatabase::create().expect("test database initializes");
    let node = AuthorizedNode {
        node_id: "recipient".into(),
        key_fingerprint: "recipient-fingerprint".into(),
        signing_public_key: "recipient-signing".into(),
        encryption_public_key: "recipient-key".into(),
        transport_public_key: None,
        grant_epoch: 1,
        status: NodeStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: "2026-08-31T00:00:00.000Z:000001:recipient".into(),
    };
    let current_grant = IntegrationNodeGrant {
        integration_id: "integration".into(),
        node_id: node.node_id.clone(),
        node_encryption_key: node.encryption_public_key.clone(),
        grant_epoch: node.grant_epoch,
        status: GrantStatus::Active,
        authorized_at: node.authorized_at.clone(),
        revoked_at: None,
        revision: 1,
        hlc: "2026-08-31T00:00:00.000Z:000001:recipient".into(),
    };
    IntegrationReplicationTestFixture::seed_authorization(
        database.path(),
        std::slice::from_ref(&node),
        std::slice::from_ref(&current_grant),
        "writer",
    )
    .expect("typed authorization records seed");
    assert!(IntegrationReplicationTestFixture::node_may_receive_frames(
        database.path(),
        &node.node_id
    )
    .expect("current active grant is checked"));

    let revoked = current_grant
        .revoke(2, "2026-08-31T00:01:00Z")
        .expect("grant revokes");
    IntegrationReplicationTestFixture::upsert_grant(database.path(), &revoked, "writer")
        .expect("revoked grant persists");
    let mut another_active = current_grant;
    another_active.integration_id = "other-integration".into();
    IntegrationReplicationTestFixture::upsert_grant(database.path(), &another_active, "writer")
        .expect("other grant persists");
    assert!(!IntegrationReplicationTestFixture::node_may_receive_frames(
        database.path(),
        &node.node_id
    )
    .expect("revoked grant blocks frame authorization"));
}

#[test]
fn concurrent_refresh_acquire_has_one_fence_winner() {
    let database = TestDatabase::create().expect("test database initializes");
    IntegrationReplicationTestFixture::seed_authorized_pair(
        database.path(),
        "node-a-signing",
        "node-b-signing",
        None,
        None,
    )
    .expect("authorized pair seeds");

    let barrier = Arc::new(Barrier::new(3));
    let handles = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let path = database.path().to_path_buf();
            std::thread::spawn(move || {
                barrier.wait();
                acquire_refresh_lease(
                    &path,
                    RefreshLeaseAcquireFixture {
                        integration_id: "integration-a".into(),
                        holder_node_id: "node-a".into(),
                        credential_generation: 3,
                        now_ms: 10,
                        ttl_ms: 10,
                        expected_fencing_token: 0,
                        device_id: "node-a".into(),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();

    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("lease worker completes"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_eq!(
        results
            .iter()
            .find_map(|result| result.as_ref().ok())
            .expect("one acquisition succeeds")
            .fencing_token,
        1
    );
}

#[test]
fn credential_publication_requires_the_current_refresh_fence() {
    let database = TestDatabase::create().expect("test database initializes");
    IntegrationReplicationTestFixture::seed_authorized_pair(
        database.path(),
        "node-a-signing",
        "node-b-signing",
        None,
        None,
    )
    .expect("authorized pair seeds");
    let lease = acquire_refresh_lease(
        database.path(),
        RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 10,
            ttl_ms: 10,
            expected_fencing_token: 0,
            device_id: "node-a".into(),
        },
    )
    .expect("refresh lease is acquired");
    let envelope = IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-a".into(),
        grant_epoch: 2,
        credential_generation: lease.credential_generation,
        refresh_fencing_token: lease.fencing_token + 1,
        key_id: encryption_key_id("enc-a"),
        algorithm: "opaque-test".into(),
        nonce: "nonce-3".into(),
        ciphertext: "opaque-ciphertext-3".into(),
        authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
        issuer_node_id: "node-a".into(),
        issued_at: "2026-08-30T00:00:00Z".into(),
        revision: 3,
        hlc: "2026-08-30T00:00:00.000Z:000003:node-a".into(),
    };
    let vector_before =
        load_version_vector(database.path()).expect("version vector loads before publication");
    assert!(publish_credential_envelope(database.path(), envelope.clone(), "node-b", 15,).is_err());
    assert!(publish_credential_envelope(database.path(), envelope.clone(), "node-a", 15,).is_err());
    assert_eq!(
        load_version_vector(database.path()).expect("version vector loads after rejection"),
        vector_before
    );
    assert_eq!(
        load_credential_envelope(database.path(), "integration-a", "node-a", 3)
            .expect("rejected generation is checked"),
        None
    );

    let mut current_envelope = envelope;
    current_envelope.refresh_fencing_token = lease.fencing_token;
    publish_credential_envelope(database.path(), current_envelope.clone(), "node-a", 15)
        .expect("current fence publication succeeds");
    assert_eq!(
        load_credential_envelope(database.path(), "integration-a", "node-a", 3)
            .expect("published envelope loads"),
        Some(current_envelope)
    );
    assert!(
        load_latest_credential_envelope(database.path(), "integration-a", "node-a")
            .expect("recipient latest envelope loads")
            .is_some()
    );
    assert!(
        load_latest_credential_envelope(database.path(), "integration-a", "node-b")
            .expect("other recipient has no envelope")
            .is_none()
    );
    assert!(acquire_refresh_lease(
        database.path(),
        RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 20,
            ttl_ms: 10,
            expected_fencing_token: lease.fencing_token,
            device_id: "node-a".into(),
        },
    )
    .is_err());
    assert_eq!(
        load_refresh_lease(database.path(), "integration-a")
            .expect("refresh lease remains readable"),
        Some(lease)
    );
}

#[test]
fn credential_publication_rejects_rotated_recipient_key_without_sync_changes() {
    let database = TestDatabase::create().expect("test database initializes");
    IntegrationReplicationTestFixture::seed_authorized_pair(
        database.path(),
        "node-a-signing",
        "node-b-signing",
        None,
        None,
    )
    .expect("authorized pair seeds");

    let mut rotated_node =
        super::authorized_node("node-a", "node-a-signing", "enc-a-rotated", None);
    rotated_node.key_fingerprint = "fingerprint-node-a-rotated".into();
    rotated_node.grant_epoch += 1;
    rotated_node.revision += 1;
    rotated_node.hlc = "2026-08-31T00:00:00.000Z:000002:node-a".into();
    let rotated_grant = IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: "node-a".into(),
        node_encryption_key: rotated_node.encryption_public_key.clone(),
        grant_epoch: rotated_node.grant_epoch,
        status: GrantStatus::Active,
        authorized_at: rotated_node.authorized_at.clone(),
        revoked_at: None,
        revision: 2,
        hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
    };
    persist_node_authorization(
        database.path(),
        NodeAuthorizationOperation::Rotate,
        rotated_node.clone(),
        Some(rotated_grant),
        "node-a",
    )
    .expect("rotated node and matching grant persist");
    let lease = acquire_refresh_lease(
        database.path(),
        RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 10,
            ttl_ms: 10,
            expected_fencing_token: 0,
            device_id: "node-a".into(),
        },
    )
    .expect("refresh lease is acquired");
    let mut envelope = IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-a".into(),
        grant_epoch: rotated_node.grant_epoch,
        credential_generation: lease.credential_generation,
        refresh_fencing_token: lease.fencing_token,
        key_id: encryption_key_id("enc-a"),
        algorithm: "opaque-test".into(),
        nonce: "nonce-3".into(),
        ciphertext: "opaque-ciphertext-3".into(),
        authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
        issuer_node_id: "node-a".into(),
        issued_at: "2026-08-31T00:00:00Z".into(),
        revision: 3,
        hlc: "2026-08-31T00:00:00.000Z:000004:node-a".into(),
    };
    let vector_before = load_version_vector(database.path())
        .expect("version vector loads before stale-key publication");
    assert!(publish_credential_envelope(database.path(), envelope.clone(), "node-a", 15).is_err());
    assert_eq!(
        load_version_vector(database.path()).expect("version vector loads after rejection"),
        vector_before
    );
    assert_eq!(
        load_credential_envelope(database.path(), "integration-a", "node-a", 3)
            .expect("rejected envelope is checked"),
        None
    );

    envelope.key_id = encryption_key_id(&rotated_node.encryption_public_key);
    publish_credential_envelope(database.path(), envelope.clone(), "node-a", 15)
        .expect("envelope with current recipient key publishes");
    assert_eq!(
        load_credential_envelope(database.path(), "integration-a", "node-a", 3)
            .expect("published envelope loads"),
        Some(envelope)
    );
}

#[test]
fn refresh_lease_retry_requires_the_local_holder_and_increments_fence() {
    let database = TestDatabase::create().expect("test database initializes");
    IntegrationReplicationTestFixture::seed_authorized_pair(
        database.path(),
        "node-a-signing",
        "node-b-signing",
        None,
        None,
    )
    .expect("authorized pair seeds");
    let rejected = acquire_refresh_lease(
        database.path(),
        RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 10,
            ttl_ms: 10,
            expected_fencing_token: 0,
            device_id: "node-b".into(),
        },
    );
    assert!(rejected.is_err());

    let first = acquire_refresh_lease(
        database.path(),
        RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: 3,
            now_ms: 10,
            ttl_ms: 10,
            expected_fencing_token: 0,
            device_id: "node-a".into(),
        },
    )
    .expect("local node acquires initial lease");
    let retry = acquire_refresh_lease(
        database.path(),
        RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: "node-a".into(),
            credential_generation: first.credential_generation,
            now_ms: first.expires_at_ms,
            ttl_ms: 10,
            expected_fencing_token: first.fencing_token,
            device_id: "node-a".into(),
        },
    )
    .expect("expired unpublished generation can retry");
    assert_eq!(retry.credential_generation, first.credential_generation);
    assert_eq!(retry.fencing_token, first.fencing_token + 1);
}
