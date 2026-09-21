use crate::integration_replication::IntegrationRefreshLease as ReplicatedRefreshLease;

fn upsert_integration_refresh_lease_record(
    conn: &Connection,
    lease: &ReplicatedRefreshLease,
    device_id: &str,
    allow_equal_fence: bool,
) -> Result<(), String> {
    lease.validate().map_err(|e| e.to_string())?;
    let (generation, fence, issued) = lease_epoch(lease)?;
    let expires = sqlite_i64(lease.expires_at_ms, "expires_at_ms")?;
    let holder = load_authorized_node(conn, &lease.holder_node_id)?
        .ok_or_else(|| IntegrationContractError::Mismatch { field: "authorized_node" }.to_string())?;
    let grant = load_integration_node_grant(conn, &lease.integration_id, &lease.holder_node_id)?
        .ok_or_else(|| IntegrationContractError::Mismatch { field: "integration_node_grant" }.to_string())?;
    if holder.status != NodeStatus::Active {
        return Err(IntegrationContractError::RevokedNode.to_string());
    }
    if grant.status != GrantStatus::Active {
        return Err(IntegrationContractError::RevokedGrant.to_string());
    }
    if holder.grant_epoch != grant.grant_epoch
        || holder.encryption_public_key != grant.node_encryption_key
    {
        return Err(IntegrationContractError::Mismatch { field: "node_key_epoch" }.to_string());
    }
    persist_with_vector(
        conn,
        "integration_refresh_lease",
        lease_entity_id(&lease.integration_id),
        device_id,
        || {
            let current = conn
                .query_row(
                    "SELECT credential_generation, fencing_token FROM integration_refresh_leases
                     WHERE integration_id = ?1",
                    params![lease.integration_id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()
                .map_err(storage)?;
            if let Some((old_generation, old_fence)) = current {
                if generation < old_generation {
                    return Err(IntegrationContractError::NonMonotonic {
                        field: "credential_generation",
                    }
                    .to_string());
                }
                if fence < old_fence || (!allow_equal_fence && fence == old_fence) {
                    return Err(IntegrationContractError::NonMonotonic {
                        field: "fencing_token",
                    }
                    .to_string());
                }
            }
            conn.execute(
                "INSERT INTO integration_refresh_leases
                 (integration_id, credential_generation, holder_node_id, fencing_token,
                  issued_at_ms, expires_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(integration_id) DO UPDATE SET
                    credential_generation = excluded.credential_generation,
                    holder_node_id = excluded.holder_node_id,
                    fencing_token = excluded.fencing_token,
                    issued_at_ms = excluded.issued_at_ms,
                    expires_at_ms = excluded.expires_at_ms",
                params![
                    lease.integration_id,
                    generation,
                    lease.holder_node_id,
                    fence,
                    issued,
                    expires,
                ],
            )
            .map_err(storage)?;
            Ok(())
        },
    )
}

pub fn load_integration_refresh_lease(
    conn: &Connection,
    integration_id: &str,
) -> Result<Option<ReplicatedRefreshLease>, String> {
    let row = conn
        .query_row(
            "SELECT credential_generation, holder_node_id, fencing_token,
                    issued_at_ms, expires_at_ms FROM integration_refresh_leases
             WHERE integration_id = ?1",
            params![integration_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((generation, holder, fence, issued, expires)) = row else {
        return Ok(None);
    };
    let lease = ReplicatedRefreshLease {
        integration_id: integration_id.to_string(),
        credential_generation: u64_value(generation, "credential_generation")?,
        holder_node_id: holder,
        fencing_token: u64_value(fence, "fencing_token")?,
        issued_at_ms: u64_value(issued, "issued_at_ms")?,
        expires_at_ms: u64_value(expires, "expires_at_ms")?,
    };
    lease.validate().map_err(|e| e.to_string())?;
    Ok(Some(lease))
}
