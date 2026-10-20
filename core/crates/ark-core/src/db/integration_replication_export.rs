use super::load_integration_credential_envelope;
use crate::integration_replication::read_store::IntegrationReplicationReadStore;

use crate::integration_replication::IntegrationRefreshLease as ReplicatedRefreshLease;
use crate::integration_replication::{
    GrantStatus as ExportGrantStatus, IntegrationReplicationChange as ExportChange,
    IntegrationReplicationEntity as ExportEntity, NodeStatus as ExportNodeStatus,
};

/// Export integration state for a peer without routing envelopes through generic sync.
pub fn export_integration_replication<S: IntegrationReplicationReadStore>(
    conn: &S,
    integration_id: &str,
    target_node_id: &str,
) -> Result<Vec<ExportChange>, String> {
    if integration_id.trim().is_empty() {
        return Err("integration_id must not be empty".into());
    }
    if target_node_id.trim().is_empty() {
        return Err("target_node_id must not be empty".into());
    }

    let mut changes = Vec::<ExportChange>::new();
    if let Some(configuration) = conn.configuration(integration_id)? {
        changes.push(export_change(
            conn,
            ExportEntity::IntegrationConfiguration(configuration),
        )?);
    }
    if let Some(lease) = conn.refresh_lease(integration_id)? {
        if lease_holder_is_current(conn, &lease)? {
            changes.push(export_change(
                conn,
                ExportEntity::IntegrationRefreshLease(lease),
            )?);
        }
    }
    if let Some(grant_epoch) = current_recipient_epoch(conn, integration_id, target_node_id)? {
        for generation in
            conn.credential_generations_for_epoch(integration_id, target_node_id, grant_epoch)?
        {
            if let Some(envelope) = load_integration_credential_envelope(
                conn,
                integration_id,
                target_node_id,
                generation,
            )? {
                changes.push(export_change(
                    conn,
                    ExportEntity::IntegrationCredentialEnvelope(envelope),
                )?);
            }
        }
    }
    Ok(changes)
}

fn lease_holder_is_current<S: IntegrationReplicationReadStore>(
    conn: &S,
    lease: &ReplicatedRefreshLease,
) -> Result<bool, String> {
    let Some(node) = conn.authorized_node(&lease.holder_node_id)? else {
        return Ok(false);
    };
    let Some(grant) = conn.grant(&lease.integration_id, &lease.holder_node_id)? else {
        return Ok(false);
    };
    Ok(node.status == ExportNodeStatus::Active
        && grant.status == ExportGrantStatus::Active
        && node.grant_epoch == grant.grant_epoch
        && node.encryption_public_key == grant.node_encryption_key)
}

fn current_recipient_epoch<S: IntegrationReplicationReadStore>(
    conn: &S,
    integration_id: &str,
    target_node_id: &str,
) -> Result<Option<u64>, String> {
    let Some(node) = conn.authorized_node(target_node_id)? else {
        return Ok(None);
    };
    let Some(grant) = conn.grant(integration_id, target_node_id)? else {
        return Ok(None);
    };
    if node.status == ExportNodeStatus::Active
        && grant.status == ExportGrantStatus::Active
        && node.grant_epoch == grant.grant_epoch
        && node.encryption_public_key == grant.node_encryption_key
    {
        Ok(Some(node.grant_epoch))
    } else {
        Ok(None)
    }
}

fn export_change<S: IntegrationReplicationReadStore>(
    conn: &S,
    entity: ExportEntity,
) -> Result<ExportChange, String> {
    let vector_hlc = load_vector_hlc(conn, &entity)?;
    let change = ExportChange { entity, vector_hlc };
    change.validate().map_err(|error| error.to_string())?;
    Ok(change)
}

fn load_vector_hlc<S: IntegrationReplicationReadStore>(
    conn: &S,
    entity: &ExportEntity,
) -> Result<String, String> {
    let vector = conn
        .version_vector()?
        .ok_or_else(|| "integration replication version vector is missing".to_string())?;
    vector
        .get(&entity_vector_key(entity))
        .cloned()
        .ok_or_else(|| {
            format!(
                "missing integration replication vector for {}",
                entity_vector_key(entity)
            )
        })
}

fn entity_vector_key(entity: &ExportEntity) -> String {
    ExportChange {
        entity: entity.clone(),
        vector_hlc: "placeholder".into(),
    }
    .vector_key()
}
