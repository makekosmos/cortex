use std::path::Path;

use ark_application::sync_policy::SyncProfile;
use ark_domain::types::VersionVector;

pub struct SyncTestState;

impl SyncTestState {
    pub fn save_local_profile(
        path: &Path,
        expected_revision: Option<i64>,
    ) -> Result<SyncProfile, String> {
        let connection = open(path)?;
        crate::data_platform::upsert_sync_profile(
            &connection,
            "device-local",
            "local-policy",
            "Local policy",
            true,
            expected_revision,
            &[],
        )
        .map_err(|error| error.to_string())
    }

    pub fn add_removed_peer(path: &Path, peer_id: &str) -> Result<(), String> {
        let connection = open(path)?;
        let mut peers: Vec<String> =
            crate::sync_kv::get_sync_kv(&connection, "sync.removed_peers")?
                .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
                .transpose()?
                .unwrap_or_default();
        if !peers.iter().any(|peer| peer == peer_id) {
            peers.push(peer_id.to_string());
        }
        let encoded = serde_json::to_string(&peers).map_err(|error| error.to_string())?;
        crate::sync_kv::set_sync_kv(&connection, "sync.removed_peers", &encoded)
    }

    pub fn save_version_vector(path: &Path, vector: &VersionVector) -> Result<(), String> {
        let connection = open(path)?;
        let encoded = serde_json::to_string(vector).map_err(|error| error.to_string())?;
        crate::sync_kv::set_sync_kv(&connection, crate::VERSION_VECTOR_KEY, &encoded)
    }

    pub fn load_version_vector(path: &Path) -> Result<VersionVector, String> {
        let connection = open(path)?;
        crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)?
            .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    pub fn connection_is_autocommit(path: &Path) -> Result<bool, String> {
        let connection = open(path)?;
        Ok(connection.is_autocommit())
    }
}

fn open(path: &Path) -> Result<rusqlite::Connection, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "sync test database path is not UTF-8".to_string())?;
    crate::open::open_db(path)
}
