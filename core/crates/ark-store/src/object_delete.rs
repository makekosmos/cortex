use crate::object_write::SqliteObjectWriteSession;
use ark_application::object_mutation::{ObjectDeleteSession, ObjectRevisionKind};
use ark_application::object_write::ObjectWriteSession;
use rusqlite::Connection;

pub struct SqliteObjectDeleteSession<'connection> {
    connection: &'connection Connection,
}

impl<'connection> SqliteObjectDeleteSession<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }
}

impl ObjectDeleteSession for SqliteObjectDeleteSession<'_> {
    fn object_snapshot(
        &self,
        id: &str,
    ) -> Result<ark_domain::object_write::ObjectWriteSnapshot, String> {
        SqliteObjectWriteSession::new(self.connection).object_snapshot(id)
    }

    fn object_type_id(&self, id: &str) -> Result<Option<String>, String> {
        Ok(crate::objects::object_type_identity(self.connection, id)?
            .map(|identity| identity.type_id))
    }

    fn delete_object(&self, id: &str) -> Result<(), String> {
        crate::objects::delete_object(self.connection, id)
    }

    fn record_delete(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
        type_id: Option<&str>,
    ) -> Result<String, String> {
        let hlc = crate::sync_versions::bump_sync_version_vector(
            self.connection,
            kind.as_str(),
            id,
            device_id,
            true,
        )?;
        crate::sync_tombstones::record(self.connection, kind.as_str(), id, type_id, &hlc)?;
        Ok(hlc)
    }

    fn write_object_sync_version(
        &self,
        id: &str,
        hlc: &str,
        deleted: bool,
    ) -> Result<usize, String> {
        crate::sync_versions::write_object_sync_version(self.connection, id, hlc, deleted)
    }
}
