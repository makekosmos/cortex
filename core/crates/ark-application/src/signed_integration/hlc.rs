use ark_domain::integration_replication::{
    IntegrationReplicationChange, IntegrationReplicationEntity,
};

pub(super) fn require_authenticated_change_hlcs(
    change: &IntegrationReplicationChange,
    origin_node_id: &str,
) -> Result<(), String> {
    require_authenticated_hlc(&change.vector_hlc, origin_node_id)?;
    match &change.entity {
        IntegrationReplicationEntity::IntegrationConfiguration(value) => {
            require_authenticated_hlc(&value.hlc, origin_node_id)?;
        }
        IntegrationReplicationEntity::AuthorizedNode(value) => {
            require_authenticated_hlc(&value.hlc, origin_node_id)?;
        }
        IntegrationReplicationEntity::IntegrationNodeGrant(value) => {
            require_authenticated_hlc(&value.hlc, origin_node_id)?;
        }
        IntegrationReplicationEntity::IntegrationRefreshLease(_) => {}
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
            require_authenticated_hlc(&value.hlc, origin_node_id)?;
        }
    }
    Ok(())
}

fn require_authenticated_hlc(hlc: &str, origin_node_id: &str) -> Result<(), String> {
    let parsed = ark_domain::hlc::HLC::from_string(hlc);
    if parsed.wall_time.is_empty() || parsed.device_id.is_empty() || parsed.to_string() != hlc {
        return Err("integration HLC is not canonical".into());
    }
    if parsed.device_id != origin_node_id {
        return Err("integration HLC device does not match signed origin".into());
    }
    let wall_time = chrono::DateTime::parse_from_rfc3339(&parsed.wall_time)
        .map_err(|_| "integration HLC wall time is invalid".to_string())?;
    let max_allowed = chrono::Utc::now() + chrono::Duration::minutes(5);
    if wall_time.with_timezone(&chrono::Utc) > max_allowed {
        return Err("integration HLC is too far in the future".into());
    }
    Ok(())
}
