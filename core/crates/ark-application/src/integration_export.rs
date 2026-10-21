use ark_domain::integration_replication::{
    GrantStatus, IntegrationReplicationChange, IntegrationReplicationEntity, NodeStatus,
};

use crate::integration_replication_read::IntegrationReplicationReadPort;

pub fn export_integration_replication<S: IntegrationReplicationReadPort>(
    store: &S,
    integration_id: &str,
    target_node_id: &str,
) -> Result<Vec<IntegrationReplicationChange>, String> {
    if integration_id.trim().is_empty() {
        return Err("integration_id must not be empty".into());
    }
    if target_node_id.trim().is_empty() {
        return Err("target_node_id must not be empty".into());
    }

    let mut changes = Vec::new();
    if let Some(configuration) = store.configuration(integration_id)? {
        changes.push(export_change(
            store,
            IntegrationReplicationEntity::IntegrationConfiguration(configuration),
        )?);
    }
    if let Some(lease) = store.refresh_lease(integration_id)? {
        if lease_holder_is_current(store, &lease)? {
            changes.push(export_change(
                store,
                IntegrationReplicationEntity::IntegrationRefreshLease(lease),
            )?);
        }
    }
    if let Some(grant_epoch) = current_recipient_epoch(store, integration_id, target_node_id)? {
        for generation in
            store.credential_generations_for_epoch(integration_id, target_node_id, grant_epoch)?
        {
            if let Some(envelope) =
                store.credential_envelope(integration_id, target_node_id, generation)?
            {
                changes.push(export_change(
                    store,
                    IntegrationReplicationEntity::IntegrationCredentialEnvelope(envelope),
                )?);
            }
        }
    }
    Ok(changes)
}

fn lease_holder_is_current<S: IntegrationReplicationReadPort>(
    store: &S,
    lease: &ark_domain::integration_replication::IntegrationRefreshLease,
) -> Result<bool, String> {
    let Some(node) = store.authorized_node(&lease.holder_node_id)? else {
        return Ok(false);
    };
    let Some(grant) = store.grant(&lease.integration_id, &lease.holder_node_id)? else {
        return Ok(false);
    };
    Ok(node.status == NodeStatus::Active
        && grant.status == GrantStatus::Active
        && node.grant_epoch == grant.grant_epoch
        && node.encryption_public_key == grant.node_encryption_key)
}

fn current_recipient_epoch<S: IntegrationReplicationReadPort>(
    store: &S,
    integration_id: &str,
    target_node_id: &str,
) -> Result<Option<u64>, String> {
    let Some(node) = store.authorized_node(target_node_id)? else {
        return Ok(None);
    };
    let Some(grant) = store.grant(integration_id, target_node_id)? else {
        return Ok(None);
    };
    if node.status == NodeStatus::Active
        && grant.status == GrantStatus::Active
        && node.grant_epoch == grant.grant_epoch
        && node.encryption_public_key == grant.node_encryption_key
    {
        Ok(Some(node.grant_epoch))
    } else {
        Ok(None)
    }
}

fn export_change<S: IntegrationReplicationReadPort>(
    store: &S,
    entity: IntegrationReplicationEntity,
) -> Result<IntegrationReplicationChange, String> {
    let vector = store
        .version_vector()?
        .ok_or_else(|| "integration replication version vector is missing".to_string())?;
    let change = IntegrationReplicationChange {
        entity,
        vector_hlc: String::new(),
    };
    let vector_hlc = vector.get(&change.vector_key()).cloned().ok_or_else(|| {
        format!(
            "missing integration replication vector for {}",
            change.vector_key()
        )
    })?;
    let change = IntegrationReplicationChange {
        vector_hlc,
        ..change
    };
    change.validate().map_err(|error| error.to_string())?;
    Ok(change)
}
