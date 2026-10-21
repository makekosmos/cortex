use ark_application::sync_storage::ReplayNotice;
use ark_application::type_mutation::{TypeEntityKind, TypeMutationSession};
use ark_domain::canonical_type::TypeRegistration;
use ark_domain::types::ObjectType;
use rusqlite::Transaction;

pub(crate) struct SqliteTypeMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteTypeMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl TypeMutationSession for SqliteTypeMutationSession<'_, '_> {
    fn register_type(&self, registration: &TypeRegistration) -> Result<Vec<ReplayNotice>, String> {
        crate::type_registry::register_type(self.transaction, registration)
    }

    fn upsert_object_type(&self, object_type: &ObjectType) -> Result<Vec<ReplayNotice>, String> {
        crate::type_registry::upsert_object_type(self.transaction, object_type)
    }

    fn delete_object_type(&self, id: &str) -> Result<(), String> {
        crate::type_registry::delete_object_type(self.transaction, id)
    }

    fn record_local_revision(
        &self,
        kind: TypeEntityKind,
        id: &str,
        device_id: Option<&str>,
        deleted: bool,
    ) -> Result<String, String> {
        let device_id = device_id
            .filter(|value| !value.trim().is_empty())
            .map(str::trim)
            .unwrap_or("ark-core-rpc-local");
        let entity_type = match kind {
            TypeEntityKind::ObjectType => "object_type",
            TypeEntityKind::ObjectLink => "object_link",
        };
        let hlc = crate::sync_versions::bump_sync_version_vector(
            self.transaction,
            entity_type,
            id,
            device_id,
            deleted,
        )?;
        if deleted {
            crate::sync_tombstones::record(self.transaction, entity_type, id, None, &hlc)?;
        } else {
            crate::sync_tombstones::delete(self.transaction, id)?;
        }
        Ok(hlc)
    }
}
