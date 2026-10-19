use crate::application::type_mutation::{TypeEntityKind, TypeMutationSession};
use crate::canonical_types::pending::ReplayNotice;
use crate::db;
use crate::type_registry::TypeRegistration;
use crate::types::{ObjectLink, ObjectType};
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
        db::upsert_object_type(self.transaction, object_type)
    }

    fn delete_object_type(&self, id: &str) -> Result<(), String> {
        db::delete_object_type(self.transaction, id)
    }

    fn upsert_object_link(&self, object_link: &ObjectLink) -> Result<(), String> {
        db::upsert_object_link(self.transaction, object_link)
    }

    fn delete_object_link(&self, id: &str) -> Result<(), String> {
        db::delete_object_link(self.transaction, id)
    }

    fn other_object_link_ids(
        &self,
        source_object_id: &str,
        link_type: &str,
        except_id: &str,
    ) -> Result<Vec<String>, String> {
        db::list_other_object_link_ids(self.transaction, source_object_id, link_type, except_id)
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
        let hlc =
            db::bump_sync_version_vector(self.transaction, entity_type, id, device_id, deleted)?;
        if deleted {
            ark_store::sync_tombstones::record(self.transaction, entity_type, id, None, &hlc)?;
        } else {
            ark_store::sync_tombstones::delete(self.transaction, id)?;
        }
        Ok(hlc)
    }
}
