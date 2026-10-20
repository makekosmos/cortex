use std::path::Path;

use ark_domain::types::VersionVector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPersistenceSnapshot {
    pub tombstones: Vec<(String, String)>,
    pub version_vector: VersionVector,
}

/// Reads sync metadata written by Core's replace-object-link service tests.
pub struct SyncPersistenceFixture;

impl SyncPersistenceFixture {
    pub fn inspect(path: &Path) -> Result<SyncPersistenceSnapshot, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "sync persistence path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let tombstones = crate::sync_tombstones::load_all(&connection)?
            .into_iter()
            .map(|tombstone| (tombstone.id, tombstone.entity_type))
            .collect();
        let version_vector = crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)?
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()?
            .unwrap_or_default();
        Ok(SyncPersistenceSnapshot {
            tombstones,
            version_vector,
        })
    }
}
