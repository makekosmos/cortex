use crate::integration_replication::{
    verify_reserve_and_apply_signed_sync, IntegrationReplicationChange,
    IntegrationReplicationEntity, SignedSyncEnvelope, SignedSyncError,
};
use std::collections::HashSet;

/// Applies an authenticated integration batch and its typed vector updates atomically.
pub fn apply_signed_integration_changes(
    conn: &Connection,
    envelope: &SignedSyncEnvelope,
    expected_space_id: &str,
    expected_recipient_node_id: &str,
    authenticated_transport_public_key: Option<&str>,
    reserved_at_ms: u64,
) -> Result<(), SignedSyncError> {
    if !conn.is_autocommit() {
        return Err(SignedSyncError::Storage(
            "signed integration apply requires an autocommit connection".into(),
        ));
    }
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| SignedSyncError::Storage(error.to_string()))?;
    let result = (|| {
        let origin = load_authorized_node(conn, &envelope.origin_node_id)
            .map_err(SignedSyncError::Storage)?
            .ok_or(SignedSyncError::UnknownOrigin)?;
        if authenticated_transport_public_key.is_some()
            && origin.transport_public_key.as_deref() != authenticated_transport_public_key
        {
            return Err(SignedSyncError::TransportMismatch);
        }
        let recipient = load_authorized_node(conn, expected_recipient_node_id)
            .map_err(SignedSyncError::Storage)?
            .ok_or(SignedSyncError::UnauthorizedRecipient)?;
        if recipient.status != crate::integration_replication::NodeStatus::Active {
            return Err(SignedSyncError::UnauthorizedRecipient);
        }
        verify_reserve_and_apply_signed_sync(
            conn,
            envelope,
            expected_space_id,
            expected_recipient_node_id,
            &origin,
            reserved_at_ms,
            |conn, verified| {
                require_verified_integration_access(conn, verified, &origin, &recipient)?;
                apply_verified_batch(conn, verified, reserved_at_ms)
            },
        )
    })();
    match result {
        Ok(()) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(SignedSyncError::Storage(error.to_string()))
            }
        },
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}
fn apply_verified_batch(
    conn: &Connection,
    verified: &crate::integration_replication::VerifiedSyncEnvelope,
    now_ms: u64,
) -> Result<(), String> {
    let mut changes = verified.payload.clone();
    changes.sort_by_key(|change| (change.dependency_rank(), change.vector_key()));
    let mut keys = HashSet::with_capacity(changes.len());
    for change in changes {
        change.validate().map_err(|error| error.to_string())?;
        require_authenticated_change_hlcs(&change, &verified.origin_node_id)?;
        let key = change.vector_key();
        if !keys.insert(key.clone()) {
            return Err(format!("duplicate integration vector key: {key}"));
        }
        if !change_requires_apply(conn, &change, &key)? {
            continue;
        }
        apply_change(
            conn,
            &change,
            &verified.origin_node_id,
            &verified.recipient_node_id,
            now_ms,
        )?;
        set_integration_vector(
            conn,
            key,
            change.vector_hlc,
            matches!(
                &change.entity,
                IntegrationReplicationEntity::IntegrationCredentialEnvelope(_)
            ),
        )?;
    }
    Ok(())
}

fn apply_change(
    conn: &Connection,
    change: &IntegrationReplicationChange,
    origin_node_id: &str,
    recipient_node_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    if change_already_applied(conn, &change.entity)? {
        return Ok(());
    }
    if let Some(integration_id) = change_integration_id(change) {
        if load_integration_configuration(conn, integration_id)?.is_none()
            && !matches!(
                &change.entity,
                IntegrationReplicationEntity::IntegrationConfiguration(_)
            )
        {
            return Err("integration configuration dependency is missing".into());
        }
    }
    match &change.entity {
        IntegrationReplicationEntity::IntegrationConfiguration(value) => {
            upsert_integration_configuration(conn, value, origin_node_id)
        }
        IntegrationReplicationEntity::AuthorizedNode(value) => {
            upsert_authorized_node(conn, value, origin_node_id)
        }
        IntegrationReplicationEntity::IntegrationNodeGrant(value) => {
            upsert_integration_node_grant(conn, value, origin_node_id)
        }
        IntegrationReplicationEntity::IntegrationRefreshLease(value) => {
            if value.holder_node_id != origin_node_id {
                return Err("refresh lease holder does not match signed origin".into());
            }
            upsert_integration_refresh_lease_record(conn, value, origin_node_id, true)
        }
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
            if value.recipient_node_id != recipient_node_id {
                return Err("credential envelope recipient does not match signed frame".into());
            }
            if value.issuer_node_id != origin_node_id {
                return Err("credential envelope issuer does not match signed origin".into());
            }
            upsert_integration_credential_envelope_record(conn, value, origin_node_id, now_ms)
        }
    }
}

fn change_already_applied(
    conn: &Connection,
    entity: &IntegrationReplicationEntity,
) -> Result<bool, String> {
    match entity {
        IntegrationReplicationEntity::IntegrationConfiguration(value) => Ok(
            load_integration_configuration(conn, &value.integration_id)?.as_ref() == Some(value),
        ),
        IntegrationReplicationEntity::AuthorizedNode(value) => {
            Ok(load_authorized_node(conn, &value.node_id)?.as_ref() == Some(value))
        }
        IntegrationReplicationEntity::IntegrationNodeGrant(value) => Ok(
            load_integration_node_grant(conn, &value.integration_id, &value.node_id)?.as_ref()
                == Some(value),
        ),
        IntegrationReplicationEntity::IntegrationRefreshLease(value) => Ok(
            load_integration_refresh_lease(conn, &value.integration_id)?.as_ref() == Some(value),
        ),
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
            Ok(load_integration_credential_envelope(
                conn,
                &value.integration_id,
                &value.recipient_node_id,
                value.credential_generation,
            )?
            .as_ref()
                == Some(value))
        }
    }
}

fn change_integration_id(change: &IntegrationReplicationChange) -> Option<&str> {
    match &change.entity {
        IntegrationReplicationEntity::IntegrationConfiguration(value) => {
            Some(&value.integration_id)
        }
        IntegrationReplicationEntity::AuthorizedNode(_) => None,
        IntegrationReplicationEntity::IntegrationNodeGrant(value) => Some(&value.integration_id),
        IntegrationReplicationEntity::IntegrationRefreshLease(value) => Some(&value.integration_id),
        IntegrationReplicationEntity::IntegrationCredentialEnvelope(value) => {
            Some(&value.integration_id)
        }
    }
}

fn vector_requires_change(conn: &Connection, key: &str, remote_hlc: &str) -> Result<bool, String> {
    let vector = load_version_vector(conn)?;
    Ok(vector
        .get(key)
        .is_none_or(|local_hlc| HLC::is_newer(remote_hlc, local_hlc)))
}

fn change_requires_apply(
    conn: &Connection,
    change: &IntegrationReplicationChange,
    key: &str,
) -> Result<bool, String> {
    let IntegrationReplicationEntity::IntegrationCredentialEnvelope(remote) = &change.entity else {
        return vector_requires_change(conn, key, &change.vector_hlc);
    };
    let Some(current) = load_integration_credential_envelope(
        conn,
        &remote.integration_id,
        &remote.recipient_node_id,
        remote.credential_generation,
    )?
    else {
        return Ok(true);
    };
    if &current == remote {
        return vector_requires_change(conn, key, &change.vector_hlc);
    }
    Ok(envelope_version_is_newer(
        &remote.hlc,
        &remote.issuer_node_id,
        &current.hlc,
        &current.issuer_node_id,
    ))
}

fn set_integration_vector(
    conn: &Connection,
    key: String,
    hlc: String,
    preserve_newer: bool,
) -> Result<(), String> {
    let mut vector = load_version_vector(conn)?;
    if preserve_newer
        && vector
            .get(&key)
            .is_some_and(|current| !HLC::is_newer(&hlc, current))
    {
        return Ok(());
    }
    vector.insert(key, hlc);
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|error| error.to_string())?,
    )
}

fn load_version_vector(conn: &Connection) -> Result<VersionVector, String> {
    get_sync_kv(conn, VERSION_VECTOR_KEY)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn require_authenticated_change_hlcs(
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
    let parsed = HLC::from_string(hlc);
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
