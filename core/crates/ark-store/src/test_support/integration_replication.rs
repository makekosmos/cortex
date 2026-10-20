use std::path::Path;

use crate::writer::SqliteWriter;
use ark_application::integration_admin::{IntegrationAdminWrite, RefreshLeaseAcquire};

#[derive(Debug, Clone)]
pub struct RefreshLeaseAcquireFixture {
    pub integration_id: String,
    pub holder_node_id: String,
    pub credential_generation: u64,
    pub now_ms: u64,
    pub ttl_ms: u64,
    pub expected_fencing_token: u64,
    pub device_id: String,
}

/// Acquires a refresh lease through the same Store transaction and application
/// policy used by production integration administration.
pub fn acquire_refresh_lease(
    path: &Path,
    request: RefreshLeaseAcquireFixture,
) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "integration fixture path is not valid UTF-8".to_string())?;
    let mut writer = SqliteWriter::open(path)?;
    writer
        .apply_integration_admin(IntegrationAdminWrite::AcquireRefreshLease(
            RefreshLeaseAcquire {
                integration_id: request.integration_id,
                holder_node_id: request.holder_node_id,
                credential_generation: request.credential_generation,
                now_ms: request.now_ms,
                ttl_ms: request.ttl_ms,
                expected_fencing_token: request.expected_fencing_token,
                device_id: request.device_id,
            },
        ))
        .map(|_| ())
        .map_err(|error| format!("integration fixture lease acquisition failed: {error:?}"))
}
