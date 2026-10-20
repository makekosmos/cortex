use std::path::Path;

use ark_domain::types::VersionVector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacedObjectLinkSyncState {
    pub tombstones: Vec<(String, String)>,
    pub version_vector: VersionVector,
}

/// Reads sync metadata written by Core's replace-object-link service tests.
pub struct ReplaceObjectLinkFixture;

impl ReplaceObjectLinkFixture {
    pub fn inspect(path: &Path) -> Result<ReplacedObjectLinkSyncState, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "replace-object-link path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let tombstones = crate::sync_tombstones::load_all(&connection)?
            .into_iter()
            .map(|tombstone| (tombstone.id, tombstone.entity_type))
            .collect();
        let version_vector = crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)?
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()?
            .unwrap_or_default();
        Ok(ReplacedObjectLinkSyncState {
            tombstones,
            version_vector,
        })
    }
}
