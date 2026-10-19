use crate::application::task_mutation::TaskMutationSession;
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ObjectLink, SyncEntity};
use rusqlite::Connection;

pub(crate) struct SqliteTaskMutationSession<'connection> {
    connection: &'connection Connection,
    object_write: ark_store::object_write::SqliteObjectWriteSession<'connection>,
}

impl<'connection> SqliteTaskMutationSession<'connection> {
    pub(crate) fn new(connection: &'connection Connection) -> Self {
        Self {
            connection,
            object_write: ark_store::object_write::SqliteObjectWriteSession::new(connection),
        }
    }
}

impl TaskMutationSession for SqliteTaskMutationSession<'_> {
    fn current_revision(&self, id: &str) -> Result<Option<String>, String> {
        ark_application::object_write::ObjectWriteSession::current_revision(&self.object_write, id)
    }

    fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        ark_application::object_write::ObjectWriteSession::record_remote_revision(
            &self.object_write,
            id,
            source_hlc,
        )
    }

    fn ensure_legacy_task_write_allowed(&self, id: &str) -> Result<(), String> {
        ark_store::task_merge::ensure_legacy_task_write_allowed(self.connection, id)
    }

    fn persist_task(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String> {
        ark_application::object_write::ObjectWriteSession::persist_object(
            &self.object_write,
            object,
            links,
        )
    }

    fn record_task_upsert(&self, id: &str, device_id: &str) -> Result<String, String> {
        ark_application::object_write::ObjectWriteSession::record_local_revision(
            &self.object_write,
            id,
            Some(device_id),
        )
    }

    fn record_task_link_upsert(&self, id: &str, device_id: &str) -> Result<String, String> {
        ark_store::sync_versions::bump_local_revision(
            self.connection,
            ark_store::sync_versions::LocalRevisionTarget::ObjectLink,
            id,
            device_id,
        )
    }

    fn task_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        ark_application::object_write::ObjectWriteSession::object_snapshot(&self.object_write, id)
    }

    fn task_type_id(&self, id: &str) -> Result<Option<String>, String> {
        ark_store::objects::get_object(self.connection, id)
            .map(|object| object.map(|object| object.type_id))
    }

    fn legacy_task_revision(&self, id: &str) -> Result<Option<String>, String> {
        ark_store::sync_versions::stored_object_revision(self.connection, id)
    }

    fn record_legacy_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        ark_store::sync_versions::record_remote_object_revision(
            self.connection,
            id,
            source_hlc,
            false,
        )
    }

    fn record_canonical_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        ark_store::sync_versions::record_remote_object_revision(
            self.connection,
            id,
            source_hlc,
            false,
        )
    }

    fn hard_delete_remote_task(&self, entity: &SyncEntity) -> Result<(), String> {
        ark_store::objects::delete_object(self.connection, &entity.id)?;
        ark_store::sync_versions::record_remote_object_deletion(self.connection, entity)
    }

    fn commit_local_task_upsert(
        &self,
        object: &ArkObject,
        links: &[ObjectLink],
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::db::task_merge::commit_local_task_upsert(
            self.connection,
            object,
            links,
            device_id,
            expected_snapshot,
        )
    }

    fn commit_local_task_delete(
        &self,
        id: &str,
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
        hard_delete: bool,
    ) -> Result<String, String> {
        crate::db::task_merge::commit_local_task_delete(
            self.connection,
            id,
            device_id,
            expected_snapshot,
            hard_delete,
        )
    }
}
