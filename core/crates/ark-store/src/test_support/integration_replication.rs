use std::path::Path;

use crate::writer::SqliteWriter;
use ark_application::integration_admin::{IntegrationAdminWrite, RefreshLeaseAcquire};
use ark_domain::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationConfiguration, IntegrationNodeGrant, NodeStatus,
};

#[derive(Debug, Clone)]
pub struct RefreshLeaseAcquireFixture {
    pub integration_id: String,
    pub holder_node_id: String,
    pub credential_generation: u64,
    pub now_ms: u64,
    pub ttl_ms: u64,
    pub expected_fencing_token: u64,
    pub device_id: String,
}

/// Acquires a refresh lease through the same Store transaction and application
/// policy used by production integration administration.
pub fn acquire_refresh_lease(
    path: &Path,
    request: RefreshLeaseAcquireFixture,
) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let mut writer = SqliteWriter::open(path)?;
    writer
        .apply_integration_admin(IntegrationAdminWrite::AcquireRefreshLease(
            RefreshLeaseAcquire {
                integration_id: request.integration_id,
                holder_node_id: request.holder_node_id,
                credential_generation: request.credential_generation,
                now_ms: request.now_ms,
                ttl_ms: request.ttl_ms,
                expected_fencing_token: request.expected_fencing_token,
                device_id: request.device_id,
            },
        ))
        .map(|_| ())
        .map_err(|error| format!("integration fixture lease acquisition failed: {error:?}"))
}

pub struct IntegrationReplicationTestFixture;

impl IntegrationReplicationTestFixture {
    pub fn seed_authorized_pair(
        path: &Path,
        origin_signing_key: &str,
        recipient_signing_key: &str,
        origin_transport_key: Option<&str>,
        recipient_transport_key: Option<&str>,
    ) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let origin = authorized_node("node-a", origin_signing_key, "enc-a", origin_transport_key);
        let recipient = authorized_node(
            "node-b",
            recipient_signing_key,
            "enc-b",
            recipient_transport_key,
        );
        crate::integration_replication::upsert_authorized_node(&connection, &origin, "bootstrap")?;
        crate::integration_replication::upsert_authorized_node(
            &connection,
            &recipient,
            "bootstrap",
        )?;
        crate::integration_replication::upsert_integration_node_grant(
            &connection,
            &integration_grant("node-a", "enc-a"),
            "bootstrap",
        )?;
        crate::integration_replication::upsert_integration_node_grant(
            &connection,
            &integration_grant("node-b", "enc-b"),
            "bootstrap",
        )?;
        Ok(())
    }

    pub fn load_configuration(
        path: &Path,
        integration_id: &str,
    ) -> Result<Option<IntegrationConfiguration>, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::integration_replication::load_integration_configuration(&connection, integration_id)
    }
}

fn authorized_node(
    node_id: &str,
    signing_public_key: &str,
    encryption_public_key: &str,
    transport_public_key: Option<&str>,
) -> AuthorizedNode {
    AuthorizedNode {
        node_id: node_id.into(),
        key_fingerprint: format!("fingerprint-{node_id}"),
        signing_public_key: signing_public_key.into(),
        encryption_public_key: encryption_public_key.into(),
        transport_public_key: transport_public_key.map(str::to_owned),
        grant_epoch: 2,
        status: NodeStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revocation_epoch: None,
        revision: 1,
        hlc: format!("2026-08-31T00:00:00.000Z:000001:{node_id}"),
    }
}

fn integration_grant(node_id: &str, encryption_key: &str) -> IntegrationNodeGrant {
    IntegrationNodeGrant {
        integration_id: "integration-a".into(),
        node_id: node_id.into(),
        node_encryption_key: encryption_key.into(),
        grant_epoch: 2,
        status: GrantStatus::Active,
        authorized_at: "2026-08-31T00:00:00Z".into(),
        revoked_at: None,
        revision: 1,
        hlc: format!("2026-08-31T00:00:00.000Z:000002:{node_id}"),
    }
}
