use ark_application::task_mutation::TaskMutationSession;
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ObjectLink, SyncEntity};
use rusqlite::Connection;

pub struct SqliteTaskMutationSession<'connection> {
    connection: &'connection Connection,
    object_write: crate::object_write::SqliteObjectWriteSession<'connection>,
}

impl<'connection> SqliteTaskMutationSession<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self {
            connection,
            object_write: crate::object_write::SqliteObjectWriteSession::new(connection),
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
        crate::sync_versions::bump_local_revision(
            self.connection,
            crate::sync_versions::LocalRevisionTarget::ObjectLink,
            id,
            device_id,
        )
    }

    fn task_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        ark_application::object_write::ObjectWriteSession::object_snapshot(&self.object_write, id)
    }

    fn task_type_id(&self, id: &str) -> Result<Option<String>, String> {
        crate::objects::get_object(self.connection, id)
            .map(|object| object.map(|object| object.type_id))
    }

    fn record_canonical_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        crate::sync_versions::record_remote_object_revision(self.connection, id, source_hlc, false)
    }

    fn hard_delete_remote_task(&self, entity: &SyncEntity) -> Result<(), String> {
        crate::objects::delete_object(self.connection, &entity.id)?;
        crate::sync_versions::record_remote_object_deletion(self.connection, entity)
    }

    fn commit_local_task_upsert(
        &self,
        object: &ArkObject,
        links: &[ObjectLink],
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::task_merge::commit_local_task_upsert(
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
        crate::task_merge::commit_local_task_delete(
            self.connection,
            id,
            device_id,
            expected_snapshot,
            hard_delete,
        )
    }
}
