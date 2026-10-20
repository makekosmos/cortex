use ark_application::tracked_app_mutation::{TrackedAppMutation, TrackedAppMutationSession};
use ark_domain::types::{SyncEntity, TrackedApp};
use rusqlite::Transaction;
use serde_json::{Map, Value};

pub(crate) struct SqliteTrackedAppMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteTrackedAppMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }

    fn upsert(&self, tracked_app: TrackedApp, device_id: &str) -> Result<SyncEntity, String> {
        crate::usage::upsert_tracked_app(self.transaction, &tracked_app)?;
        let hlc = crate::sync_versions::bump_sync_version_vector(
            self.transaction,
            "tracked_app",
            &tracked_app.id,
            device_id,
            false,
        )?;
        crate::sync_tombstones::delete(self.transaction, &tracked_app.id)?;
        let mut data = serde_json::to_value(&tracked_app)
            .map_err(|error| error.to_string())?
            .as_object()
            .cloned()
            .unwrap_or_default();
        data.remove("id");
        Ok(SyncEntity {
            entity_type: "tracked_app".into(),
            id: tracked_app.id,
            data,
            hlc,
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        })
    }

    fn delete(&self, id: String, device_id: &str) -> Result<SyncEntity, String> {
        crate::usage::delete_tracked_app(self.transaction, &id)?;
        let hlc = crate::sync_versions::bump_sync_version_vector(
            self.transaction,
            "tracked_app",
            &id,
            device_id,
            true,
        )?;
        crate::sync_tombstones::record(self.transaction, "tracked_app", &id, None, &hlc)?;
        Ok(SyncEntity {
            entity_type: "tracked_app".into(),
            id,
            data: Map::<String, Value>::new(),
            hlc,
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        })
    }
}

impl TrackedAppMutationSession for SqliteTrackedAppMutationSession<'_, '_> {
    fn apply(&self, mutation: TrackedAppMutation) -> Result<SyncEntity, String> {
        match mutation {
            TrackedAppMutation::Upsert(write) => self.upsert(write.tracked_app, &write.device_id),
            TrackedAppMutation::Delete(write) => self.delete(write.id, &write.device_id),
        }
    }
}
