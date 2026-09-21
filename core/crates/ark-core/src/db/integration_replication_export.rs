use crate::integration_replication::{
    GrantStatus as ExportGrantStatus, IntegrationReplicationChange as ExportChange,
    IntegrationReplicationEntity as ExportEntity, NodeStatus as ExportNodeStatus,
};

/// Export integration state for a peer without routing envelopes through generic sync.
pub fn export_integration_replication(
    conn: &Connection,
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
    if let Some(configuration) = load_integration_configuration(conn, integration_id)? {
        changes.push(export_change(
            conn,
            ExportEntity::IntegrationConfiguration(configuration),
        )?);
    }

    if let Some(lease) = load_integration_refresh_lease(conn, integration_id)? {
        if lease_holder_is_current(conn, &lease)? {
            changes.push(export_change(
                conn,
                ExportEntity::IntegrationRefreshLease(lease),
            )?);
        }
    }

    if target_is_current_recipient(conn, integration_id, target_node_id)? {
        let mut statement = conn
            .prepare(
                "SELECT credential_generation
                 FROM integration_credential_envelopes
                 WHERE integration_id = ?1 AND recipient_node_id = ?2
                   AND grant_epoch = (SELECT grant_epoch FROM authorized_nodes WHERE node_id = ?2)
                 ORDER BY credential_generation ASC",
            )
            .map_err(storage)?;
        let generations = statement
            .query_map(params![integration_id, target_node_id], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        drop(statement);
        for generation in generations {
            let generation = u64_value(generation, "credential_generation")?;
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

fn lease_holder_is_current(
    conn: &Connection,
    lease: &ReplicatedRefreshLease,
) -> Result<bool, String> {
    let Some(node) = load_authorized_node(conn, &lease.holder_node_id)? else {
        return Ok(false);
    };
    let Some(grant) =
        load_integration_node_grant(conn, &lease.integration_id, &lease.holder_node_id)?
    else {
        return Ok(false);
    };
    Ok(node.status == ExportNodeStatus::Active
        && grant.status == ExportGrantStatus::Active
        && node.grant_epoch == grant.grant_epoch
        && node.encryption_public_key == grant.node_encryption_key)
}

fn target_is_current_recipient(
    conn: &Connection,
    integration_id: &str,
    target_node_id: &str,
) -> Result<bool, String> {
    let Some(node) = load_authorized_node(conn, target_node_id)? else {
        return Ok(false);
    };
    let Some(grant) = load_integration_node_grant(conn, integration_id, target_node_id)? else {
        return Ok(false);
    };
    Ok(node.status == ExportNodeStatus::Active
        && grant.status == ExportGrantStatus::Active
        && node.grant_epoch == grant.grant_epoch
        && node.encryption_public_key == grant.node_encryption_key)
}

fn export_change(conn: &Connection, entity: ExportEntity) -> Result<ExportChange, String> {
    let vector_hlc = load_vector_hlc(conn, &entity)?;
    let change = ExportChange { entity, vector_hlc };
    change.validate().map_err(|error| error.to_string())?;
    Ok(change)
}

fn load_vector_hlc(conn: &Connection, entity: &ExportEntity) -> Result<String, String> {
    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?
        .ok_or_else(|| "integration replication version vector is missing".to_string())?;
    let vector: VersionVector = serde_json::from_str(&raw).map_err(storage)?;
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

fn entity_vector_key(entity: &IntegrationReplicationEntity) -> String {
    let change = ExportChange {
        entity: entity.clone(),
        vector_hlc: "placeholder".into(),
    };
    change.vector_key()
}
