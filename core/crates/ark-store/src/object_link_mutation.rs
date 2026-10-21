use ark_application::type_mutation::ObjectLinkMutationSession;
use ark_domain::types::ObjectLink;
use rusqlite::Transaction;

pub struct SqliteObjectLinkMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteObjectLinkMutationSession<'tx, 'connection> {
    pub fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl ObjectLinkMutationSession for SqliteObjectLinkMutationSession<'_, '_> {
    fn upsert_object_link(&self, object_link: &ObjectLink) -> Result<(), String> {
        crate::objects::upsert_object_link(self.transaction, object_link)
    }

    fn delete_object_link(&self, id: &str) -> Result<(), String> {
        crate::objects::delete_object_link(self.transaction, id)
    }

    fn other_object_link_ids(
        &self,
        source_object_id: &str,
        link_type: &str,
        except_id: &str,
    ) -> Result<Vec<String>, String> {
        crate::objects::list_other_object_link_ids(
            self.transaction,
            source_object_id,
            link_type,
            except_id,
        )
    }

    fn record_local_revision(
        &self,
        id: &str,
        device_id: Option<&str>,
        deleted: bool,
    ) -> Result<String, String> {
        let device_id = device_id
            .filter(|value| !value.trim().is_empty())
            .map(str::trim)
            .unwrap_or("ark-core-rpc-local");
        let hlc = crate::sync_versions::bump_sync_version_vector(
            self.transaction,
            "object_link",
            id,
            device_id,
            deleted,
        )?;
        if deleted {
            crate::sync_tombstones::record(self.transaction, "object_link", id, None, &hlc)?;
        } else {
            crate::sync_tombstones::delete(self.transaction, id)?;
        }
        Ok(hlc)
    }
}
