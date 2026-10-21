use serde::Serialize;

pub use ark_application::integration_sync::{prepare_signed_sync, SignedSyncPreparation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NodeAuthorizationOperation {
    Authorize,
    Revoke,
    Rotate,
}

/// Params of `integration.acquire_refresh_lease`, shared by the request
/// newtype and its Engine-facing serialized fields.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RefreshLeaseAcquireParams {
    pub integration_id: String,
    pub holder_node_id: String,
    pub credential_generation: u64,
    pub now_ms: u64,
    pub ttl_ms: u64,
    pub expected_fencing_token: u64,
    pub device_id: String,
}
