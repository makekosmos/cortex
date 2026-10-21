use crate::integration_replication::IntegrationReplicationReadPort;
use crate::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationCredentialEnvelope,
    IntegrationNodeGrant, IntegrationRefreshLease, NodeStatus,
};
use ark_store::test_support::{
    IntegrationReplicationReadSnapshot, IntegrationReplicationTestFixture,
    RefreshLeaseAcquireFixture,
};

struct IntegrationTestDatabase {
    fixture: IntegrationReplicationTestFixture,
}

impl IntegrationTestDatabase {
    fn create() -> Self {
        Self {
            fixture: IntegrationReplicationTestFixture::create()
                .expect("integration test database initializes"),
        }
    }

    fn path(&self) -> &std::path::Path {
        self.fixture.path()
    }
}

fn integration_test_db() -> IntegrationTestDatabase {
    IntegrationTestDatabase::create()
}

fn with_integration_replication_read<T>(
    database: &IntegrationTestDatabase,
    operation: impl FnOnce(&IntegrationReplicationReadSnapshot) -> Result<T, String>,
) -> Result<T, String> {
    operation(&database.fixture.read_snapshot()?)
}

fn integration_read_snapshot(
    database: &IntegrationTestDatabase,
) -> IntegrationReplicationReadSnapshot {
    database
        .fixture
        .read_snapshot()
        .expect("integration read snapshot loads")
}

fn upsert_integration_configuration(
    database: &IntegrationTestDatabase,
    configuration: &IntegrationConfiguration,
    device_id: &str,
) -> Result<(), String> {
    database
        .fixture
        .upsert_configuration(configuration, device_id)
}

fn upsert_authorized_node(
    database: &IntegrationTestDatabase,
    node: &AuthorizedNode,
    device_id: &str,
) -> Result<(), String> {
    database.fixture.upsert_node(node, device_id)
}

fn upsert_integration_node_grant(
    database: &IntegrationTestDatabase,
    grant: &IntegrationNodeGrant,
    device_id: &str,
) -> Result<(), String> {
    database.fixture.upsert_integration_grant(grant, device_id)
}

fn load_integration_node_grant(
    database: &IntegrationTestDatabase,
    integration_id: &str,
    node_id: &str,
) -> Result<Option<IntegrationNodeGrant>, String> {
    integration_read_snapshot(database).grant(integration_id, node_id)
}

fn current_version_vector(database: &IntegrationTestDatabase) -> VersionVector {
    integration_read_snapshot(database)
        .version_vector()
        .expect("typed version vector reads")
        .expect("typed version vector exists")
}

fn replace_version_vector_for_test(database: &IntegrationTestDatabase, vector: &VersionVector) {
    database
        .fixture
        .replace_version_vector_for_test(vector)
        .expect("typed version vector replacement succeeds");
}

fn integration_config(settings: serde_json::Value) -> IntegrationConfiguration {
    IntegrationConfiguration {
        integration_id: "integration-a".into(),
        provider: "fatsecret".into(),
        account_subject: "account-a".into(),
        public_scopes: vec!["diary.read".into()],
        public_settings: settings,
        enabled: true,
        sync_cursor: Some(1),
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn integration_node() -> AuthorizedNode {
    AuthorizedNode {
        node_id: "node-a".into(),
        key_fingerprint: "fp-a".into(),
        signing_public_key: "sign-a".into(),
        encryption_public_key: "enc-a".into(),
        transport_public_key: None,
        grant_epoch: 2,
        status: NodeStatus::Active,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}
fn integration_grant() -> IntegrationNodeGrant {
    IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: "node-a".into(),
        node_encryption_key: "enc-a".into(),
        grant_epoch: 2,
        status: GrantStatus::Active,
        authorized_at: "2026-08-30T00:00:00Z".into(),
        revoked_at: None,
        revision: 1,
        hlc: "2026-08-30T00:00:00.000Z:000001:node-a".into(),
    }
}

fn integration_envelope(generation: u64) -> IntegrationCredentialEnvelope {
    IntegrationCredentialEnvelope {
        integration_id: "integration-a".into(),
        recipient_node_id: "node-a".into(),
        grant_epoch: 2,
        credential_generation: generation,
        refresh_fencing_token: 1,
        key_id: crate::integration_replication::encryption_key_id("enc-a"),
        algorithm: "opaque-test".into(),
        nonce: format!("nonce-{generation}"),
        ciphertext: format!("opaque-ciphertext-{generation}"),
        authenticated_metadata: Some(serde_json::json!({"content_type": "oauth"})),
        issuer_node_id: "node-a".into(),
        issued_at: "2026-08-30T00:00:00Z".into(),
        revision: generation,
        hlc: format!("2026-08-30T00:00:00.000Z:00000{generation}:node-a"),
    }
}

fn acquire_integration_test_lease(
    database: &IntegrationTestDatabase,
    device_id: &str,
) -> IntegrationRefreshLease {
    acquire_test_lease(database, "node-a", 3, 10, 10, 0, device_id)
}

fn acquire_test_lease(
    database: &IntegrationTestDatabase,
    holder_node_id: &str,
    credential_generation: u64,
    now_ms: u64,
    ttl_ms: u64,
    expected_fencing_token: u64,
    device_id: &str,
) -> IntegrationRefreshLease {
    database
        .fixture
        .acquire_refresh_lease(RefreshLeaseAcquireFixture {
            integration_id: "integration-a".into(),
            holder_node_id: holder_node_id.into(),
            credential_generation,
            now_ms,
            ttl_ms,
            expected_fencing_token,
            device_id: device_id.into(),
        })
        .unwrap()
}

fn publish_test_envelope(
    database: &IntegrationTestDatabase,
    envelope: &IntegrationCredentialEnvelope,
    device_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    database
        .fixture
        .publish_credential_envelope(envelope.clone(), device_id, now_ms)
}

#[test]
fn verification_status_is_plaintext_free_and_reports_local_readiness() {
    let conn = integration_test_db();
    let empty = with_integration_replication_read(&conn, |read| {
        integration_verification_status(read, "integration-a", "node-a", 15)
    })
    .unwrap();
    assert!(!empty.integration_present);
    assert!(!empty.ready_for_collection);
    assert!(!empty.ready_for_refresh);

    upsert_integration_configuration(
        &conn,
        &integration_config(serde_json::json!({"display": {"region": "us"}})),
        "node-a",
    )
    .unwrap();
    upsert_authorized_node(&conn, &integration_node(), "node-a").unwrap();
    upsert_integration_node_grant(&conn, &integration_grant(), "node-a").unwrap();
    acquire_integration_test_lease(&conn, "node-a");
    publish_test_envelope(&conn, &integration_envelope(3), "node-a", 15).unwrap();

    let status = with_integration_replication_read(&conn, |read| {
        integration_verification_status(read, "integration-a", "node-a", 15)
    })
    .unwrap();
    assert!(status.integration_present);
    assert_eq!(status.integration_enabled, Some(true));
    assert_eq!(status.node_status, Some(NodeStatus::Active));
    assert_eq!(status.grant_status, Some(GrantStatus::Active));
    assert!(status.grant_key_matches_node);
    assert_eq!(status.envelope_generation, Some(3));
    assert!(status.envelope_available);
    assert!(status.ready_for_collection);
    assert!(!status.ready_for_refresh);
    assert!(
        with_integration_replication_read(&conn, |read| {
            integration_verification_status(read, "integration-a", "node-a", 20)
        })
        .unwrap()
        .ready_for_refresh
    );
    let encoded = serde_json::to_string(&status).unwrap();
    for forbidden in [
        "opaque-ciphertext",
        "nonce",
        "authenticated_metadata",
        "public_settings",
        "region",
    ] {
        assert!(!encoded.contains(forbidden));
    }
}
