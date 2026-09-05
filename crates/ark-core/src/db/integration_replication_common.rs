use serde::de::DeserializeOwned;

use crate::integration_replication::{
    envelope_entity_id, integration_node_id, IntegrationContractError,
    IntegrationRefreshLease as CommonIntegrationRefreshLease,
};

const SAVEPOINT: &str = "ark_integration_replication_write";

fn storage(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(storage)
}

fn parse<T: DeserializeOwned>(value: &str) -> Result<T, String> {
    serde_json::from_str(value).map_err(storage)
}

fn sqlite_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

fn u64_value(value: i64, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} is negative"))
}

fn require_newer_hlc(proposed: &str, current: &str) -> Result<(), String> {
    if HLC::is_newer(proposed, current) {
        Ok(())
    } else {
        Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string())
    }
}

fn envelope_version_is_newer(
    proposed_hlc: &str,
    proposed_issuer: &str,
    current_hlc: &str,
    current_issuer: &str,
) -> bool {
    HLC::is_newer(proposed_hlc, current_hlc)
        || (proposed_hlc == current_hlc && proposed_issuer > current_issuer)
}

fn with_savepoint<T>(conn: &Connection, operation: impl FnOnce() -> Result<T, String>)
    -> Result<T, String>
{
    conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(storage)?;
    match operation() {
        Ok(value) => {
            match conn.execute_batch(&format!("RELEASE SAVEPOINT {SAVEPOINT}")) {
                Ok(()) => Ok(value),
                Err(error) => {
                    let _ = conn.execute_batch(&format!(
                        "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
                    ));
                    Err(storage(error))
                }
            }
        }
        Err(error) => {
            let _ = conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
            ));
            Err(error)
        }
    }
}

fn persist_with_vector(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    write: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err(IntegrationContractError::EmptyField { field: "device_id" }.to_string());
    }
    with_savepoint(conn, || {
        write()?;
        let vector_id = format!("{entity_type}:{entity_id}");
        bump_sync_version_vector(conn, entity_type, &vector_id, device_id, false)?;
        Ok(())
    })
}

fn lease_entity_id(integration_id: &str) -> &str {
    integration_id
}

fn lease_epoch(lease: &CommonIntegrationRefreshLease) -> Result<(i64, i64, i64), String> {
    Ok((
        sqlite_i64(lease.credential_generation, "credential_generation")?,
        sqlite_i64(lease.fencing_token, "fencing_token")?,
        sqlite_i64(lease.issued_at_ms, "issued_at_ms")?,
    ))
}
