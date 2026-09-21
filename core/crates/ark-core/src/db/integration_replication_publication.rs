/// Acquire a refresh lease with an atomic expected-fence compare-and-swap.
#[allow(clippy::too_many_arguments)]
pub fn try_acquire_integration_refresh_lease(
    conn: &Connection,
    integration_id: &str,
    holder_node_id: &str,
    credential_generation: u64,
    now_ms: u64,
    ttl_ms: u64,
    expected_fencing_token: u64,
    device_id: &str,
) -> Result<ReplicatedRefreshLease, String> {
    if holder_node_id != device_id {
        return Err("refresh lease holder must be the local node".into());
    }
    with_immediate_integration_write(conn, || {
        let published_generation = conn
            .query_row(
                "SELECT MAX(credential_generation)
                 FROM integration_credential_envelopes WHERE integration_id = ?1",
                [integration_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(storage)?
            .map(|value| u64_value(value, "credential_generation"))
            .transpose()?;
        if published_generation.is_some_and(|current| credential_generation <= current) {
            return Err(IntegrationContractError::NonMonotonic {
                field: "credential_generation",
            }
            .to_string());
        }
        let current = load_integration_refresh_lease(conn, integration_id)?;
        let actual_fence = current.as_ref().map_or(0, |lease| lease.fencing_token);
        if actual_fence != expected_fencing_token {
            return Err(IntegrationContractError::Mismatch {
                field: "expected_fencing_token",
            }
            .to_string());
        }
        let lease = crate::integration_replication::acquire_refresh_lease(
            current.as_ref(),
            integration_id,
            holder_node_id,
            credential_generation,
            now_ms,
            ttl_ms,
        )
        .map_err(|error| error.to_string())?;
        upsert_integration_refresh_lease_record(conn, &lease, device_id, false)?;
        Ok(lease)
    })
}

/// Publish opaque credentials only while the exact lease generation and fence are current.
pub fn publish_integration_credential_envelope(
    conn: &Connection,
    envelope: &IntegrationCredentialEnvelope,
    device_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    if envelope.issuer_node_id != device_id {
        return Err("credential envelope issuer must be the local node".into());
    }
    with_immediate_integration_write(conn, || {
        upsert_integration_credential_envelope_record(conn, envelope, device_id, now_ms)
    })
}

fn with_immediate_integration_write<T>(
    conn: &Connection,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if !conn.is_autocommit() {
        return Err("integration write requires an autocommit connection".into());
    }
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    match operation() {
        Ok(value) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(value),
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(storage(error))
            }
        },
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}
