use std::path::Path;

use ark_domain::types::VersionVector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectValidationState {
    pub object_exists: bool,
    pub version_vector_contains_object: bool,
}

pub struct ObjectValidationFixture;

impl ObjectValidationFixture {
    pub fn inspect(path: &Path, object_id: &str) -> Result<ObjectValidationState, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "object validation path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let object_exists = crate::objects::get_object(&connection, object_id)?.is_some();
        let vector = crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)?
            .map(|value| serde_json::from_str::<VersionVector>(&value))
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();

        Ok(ObjectValidationState {
            object_exists,
            version_vector_contains_object: vector.contains_key(object_id),
        })
    }
}
