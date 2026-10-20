#[cfg(test)]
use crate::hlc::HLC;
#[cfg(test)]
use crate::integration_replication::IntegrationContractError;
#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
pub(super) fn storage(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
pub(super) fn envelope_version_is_newer(
    proposed_hlc: &str,
    proposed_issuer: &str,
    current_hlc: &str,
    current_issuer: &str,
) -> bool {
    HLC::is_newer(proposed_hlc, current_hlc)
        || (proposed_hlc == current_hlc && proposed_issuer > current_issuer)
}

#[cfg(test)]
pub(super) fn persist_with_vector(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    write: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err(IntegrationContractError::EmptyField { field: "device_id" }.to_string());
    }
    const SAVEPOINT: &str = "ark_integration_replication_test_write";
    conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(storage)?;
    let result = (|| {
        write()?;
        let vector_id = format!("{entity_type}:{entity_id}");
        ark_store::sync_versions::bump_sync_version_vector(
            conn,
            entity_type,
            &vector_id,
            device_id,
            false,
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch(&format!("RELEASE SAVEPOINT {SAVEPOINT}"))
            .map_err(storage),
        Err(error) => {
            conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
            ))
            .map_err(|rollback_error| {
                format!("{error}; savepoint rollback failed: {rollback_error}")
            })?;
            Err(error)
        }
    }
}

#[cfg(test)]
pub(super) fn lease_entity_id(integration_id: &str) -> &str {
    integration_id
}
