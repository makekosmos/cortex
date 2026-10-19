use crate::application::legacy_mutation::{LegacyMutationSession, LegacyObjectKind};
use crate::db;
use crate::types::{SyncEntity, TrackedApp};
use rusqlite::Transaction;
use serde_json::{Map, Value};

pub(crate) struct SqliteLegacyMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteLegacyMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl LegacyMutationSession for SqliteLegacyMutationSession<'_, '_> {
    fn write_records(
        &self,
        records: &[crate::canonical_types::compatibility::LegacyRecord],
        device_id: Option<&str>,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::canonical_types::facades::write_legacy_records(
            self.transaction,
            records,
            "rpc",
            device_id.map(str::to_owned),
        )
    }

    fn delete_legacy_object(
        &self,
        id: &str,
        kind: LegacyObjectKind,
        device_id: Option<&str>,
    ) -> Result<SyncEntity, String> {
        crate::canonical_types::facades::delete_legacy_object(
            self.transaction,
            id,
            kind.type_id(),
            device_id.map(str::to_owned),
        )
    }

    fn upsert_tracked_app(
        &self,
        tracked_app: &TrackedApp,
        device_id: &str,
    ) -> Result<SyncEntity, String> {
        ark_store::usage::upsert_tracked_app(self.transaction, tracked_app)?;
        let hlc = db::bump_sync_version_vector(
            self.transaction,
            "tracked_app",
            &tracked_app.id,
            device_id,
            false,
        )?;
        ark_store::sync_tombstones::delete(self.transaction, &tracked_app.id)?;
        let mut data = serde_json::to_value(tracked_app)
            .map_err(|error| error.to_string())?
            .as_object()
            .cloned()
            .unwrap_or_default();
        data.remove("id");
        Ok(SyncEntity {
            entity_type: "tracked_app".into(),
            id: tracked_app.id.clone(),
            data,
            hlc,
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        })
    }

    fn delete_tracked_app(&self, id: &str, device_id: &str) -> Result<SyncEntity, String> {
        ark_store::usage::delete_tracked_app(self.transaction, id)?;
        let hlc =
            db::bump_sync_version_vector(self.transaction, "tracked_app", id, device_id, true)?;
        ark_store::sync_tombstones::record(self.transaction, "tracked_app", id, None, &hlc)?;
        Ok(SyncEntity {
            entity_type: "tracked_app".into(),
            id: id.into(),
            data: Map::<String, Value>::new(),
            hlc,
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        })
    }
}
