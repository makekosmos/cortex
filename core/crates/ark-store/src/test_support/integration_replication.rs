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
    /// Seeds typed authorization records for Core's outbound validation tests.
    pub fn seed_authorization(
        path: &Path,
        nodes: &[AuthorizedNode],
        grants: &[IntegrationNodeGrant],
        device_id: &str,
    ) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        for node in nodes {
            crate::integration_replication::upsert_authorized_node(&connection, node, device_id)?;
        }
        for grant in grants {
            crate::integration_replication::upsert_integration_node_grant(
                &connection,
                grant,
                device_id,
            )?;
        }
        Ok(())
    }

    /// Replaces a typed grant record through Store's integration persistence API.
    pub fn upsert_grant(
        path: &Path,
        grant: &IntegrationNodeGrant,
        device_id: &str,
    ) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::integration_replication::upsert_integration_node_grant(&connection, grant, device_id)
    }

    /// Reports whether every persisted grant still authorizes the node's current key.
    pub fn node_may_receive_frames(path: &Path, node_id: &str) -> Result<bool, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let Some(node) =
            crate::integration_replication::load_authorized_node(&connection, node_id)?
        else {
            return Ok(false);
        };
        if node.status != NodeStatus::Active {
            return Ok(false);
        }
        crate::integration_replication::node_grants_match_current_authorization(
            &connection,
            node_id,
            node.grant_epoch,
            &node.encryption_public_key,
        )
    }

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

#[cfg(test)]
mod tests {
    use super::IntegrationReplicationTestFixture;
    use crate::test_support::TestDatabase;
    use ark_domain::integration_replication::{
        AuthorizedNode, GrantStatus, IntegrationNodeGrant, NodeStatus,
    };

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
        assert!(
            IntegrationReplicationTestFixture::node_may_receive_frames(
                database.path(),
                &node.node_id
            )
            .expect("current active grant is checked")
        );

        let revoked = current_grant
            .revoke(2, "2026-08-31T00:01:00Z")
            .expect("grant revokes");
        IntegrationReplicationTestFixture::upsert_grant(database.path(), &revoked, "writer")
            .expect("revoked grant persists");
        let mut another_active = current_grant;
        another_active.integration_id = "other-integration".into();
        IntegrationReplicationTestFixture::upsert_grant(database.path(), &another_active, "writer")
            .expect("other grant persists");
        assert!(
            !IntegrationReplicationTestFixture::node_may_receive_frames(
                database.path(),
                &node.node_id
            )
            .expect("revoked grant blocks frame authorization")
        );
    }
}
