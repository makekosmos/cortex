use std::path::Path;

use ark_domain::canonical_type::{AliasRecord, TypeRegistration};
use ark_domain::types::SyncEntity;
use rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeMutationState {
    pub registered: bool,
    pub pending_objects: i64,
    pub object_exists: bool,
}

/// Owns storage setup and inspection for service tests of type registration.
pub struct TypeMutationFixture {
    connection: Connection,
}

impl TypeMutationFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "type-mutation database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        Ok(Self { connection })
    }

    pub fn insert_pending_object(&self, entity: &SyncEntity, type_id: &str) -> Result<(), String> {
        crate::pending_objects::insert_pending_object(&self.connection, entity, type_id)
    }

    pub fn register_type(&self, registration: &TypeRegistration) -> Result<(), String> {
        crate::type_registry::register_type(&self.connection, registration).map(|_| ())
    }

    pub fn register_alias(&self, alias: &AliasRecord) -> Result<(), String> {
        crate::type_registry::register_alias(&self.connection, alias)
    }

    pub fn load_registered_type(
        &self,
        type_id: &str,
        version: &str,
    ) -> Result<Option<ark_application::type_registry::TypeGetResponse>, String> {
        crate::type_registry::get_type(&self.connection, type_id, Some(version))
    }

    pub fn reject_sync_revision_writes(&self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "CREATE TRIGGER fail_type_revision_insert BEFORE INSERT ON sync_kv
                 WHEN NEW.key = 'lan_sync.version_vector' BEGIN
                 SELECT RAISE(ABORT, 'injected type revision failure'); END;
                 CREATE TRIGGER fail_type_revision_update BEFORE UPDATE ON sync_kv
                 WHEN OLD.key = 'lan_sync.version_vector' BEGIN
                 SELECT RAISE(ABORT, 'injected type revision failure'); END;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn allow_sync_revision_writes(&self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "DROP TRIGGER fail_type_revision_insert;
                 DROP TRIGGER fail_type_revision_update;",
            )
            .map_err(|error| error.to_string())
    }

    pub fn inspect(&self, type_id: &str, entity_id: &str) -> Result<TypeMutationState, String> {
        let registered = crate::object_types::get(&self.connection, type_id)?.is_some();
        let pending_objects = crate::pending_objects::count_for_type(&self.connection, type_id)?;
        let object_exists = crate::objects::get_object(&self.connection, entity_id)?.is_some();
        Ok(TypeMutationState {
            registered,
            pending_objects,
            object_exists,
        })
    }
}
