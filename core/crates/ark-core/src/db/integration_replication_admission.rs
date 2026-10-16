use crate::integration_replication::IntegrationVerificationStatus;

pub fn integration_verification_status(
    conn: &Connection,
    integration_id: &str,
    local_node_id: &str,
    now_ms: u64,
) -> Result<IntegrationVerificationStatus, String> {
    if integration_id.trim().is_empty() || local_node_id.trim().is_empty() {
        return Err("integration and local node ids must not be empty".into());
    }
    let configuration = load_integration_configuration(conn, integration_id)?;
    let node = load_authorized_node(conn, local_node_id)?;
    let grant = load_integration_node_grant(conn, integration_id, local_node_id)?;
    let grant_key_matches_node = node
        .as_ref()
        .zip(grant.as_ref())
        .is_some_and(|(node, grant)| {
            grant.grant_epoch == node.grant_epoch
                && grant.node_encryption_key == node.encryption_public_key
        });
    let access_is_current = node.as_ref().is_some_and(|node| {
        node.status == crate::integration_replication::NodeStatus::Active
            && grant.as_ref().is_some_and(|grant| {
                grant.status == crate::integration_replication::GrantStatus::Active
                    && grant_key_matches_node
            })
    });
    let envelope = if access_is_current {
        load_latest_integration_credential_envelope(conn, integration_id, local_node_id)?
    } else {
        None
    };
    let refresh_lease = load_integration_refresh_lease(conn, integration_id)?;
    let next_generation = envelope
        .as_ref()
        .and_then(|envelope| envelope.credential_generation.checked_add(1));
    let lease_is_current = refresh_lease.as_ref().is_some_and(|lease| {
        lease.holder_node_id == local_node_id
            && lease.issued_at_ms <= now_ms
            && now_ms < lease.expires_at_ms
            && Some(lease.credential_generation) == next_generation
    });
    let lease_allows_refresh = refresh_lease
        .as_ref()
        .is_none_or(|lease| lease.expires_at_ms <= now_ms || lease_is_current);
    let ready_for_collection = configuration
        .as_ref()
        .is_some_and(|configuration| configuration.enabled)
        && access_is_current
        && envelope.is_some();

    Ok(IntegrationVerificationStatus {
        integration_id: integration_id.into(),
        local_node_id: local_node_id.into(),
        integration_present: configuration.is_some(),
        integration_enabled: configuration.as_ref().map(|value| value.enabled),
        node_status: node.as_ref().map(|value| value.status),
        node_grant_epoch: node.as_ref().map(|value| value.grant_epoch),
        grant_status: grant.as_ref().map(|value| value.status),
        grant_epoch: grant.as_ref().map(|value| value.grant_epoch),
        grant_key_matches_node,
        envelope_generation: envelope.as_ref().map(|value| value.credential_generation),
        envelope_grant_epoch: envelope.as_ref().map(|value| value.grant_epoch),
        envelope_key_id: envelope.as_ref().map(|value| value.key_id.clone()),
        envelope_available: envelope.is_some(),
        refresh_lease,
        lease_is_current,
        ready_for_collection,
        ready_for_refresh: ready_for_collection && lease_allows_refresh,
    })
}
