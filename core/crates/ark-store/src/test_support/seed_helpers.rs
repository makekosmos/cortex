use ark_domain::integration_replication::{
    AuthorizedNode, GrantStatus, IntegrationNodeGrant, NodeStatus,
};

pub(super) fn authorized_node(
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

pub(super) fn integration_grant(node_id: &str, encryption_key: &str) -> IntegrationNodeGrant {
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
