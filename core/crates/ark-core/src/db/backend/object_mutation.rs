use crate::application::object_mutation::{ObjectMutationSession, ObjectRevisionKind};
use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::types::ArkObject;
use rusqlite::Transaction;

pub(crate) struct SqliteObjectMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteObjectMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl ObjectMutationSession for SqliteObjectMutationSession<'_, '_> {
    fn persist_object(&self, object: &ArkObject) -> Result<(), String> {
        ark_store::objects::upsert_object_compat(self.transaction, object)
    }

    fn record_upsert(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
    ) -> Result<String, String> {
        let hlc = ark_store::sync_versions::bump_sync_version_vector(
            self.transaction,
            kind.as_str(),
            id,
            device_id,
            false,
        )?;
        ark_store::sync_tombstones::delete(self.transaction, id)?;
        Ok(hlc)
    }

    fn record_delete(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
        type_id: Option<&str>,
    ) -> Result<String, String> {
        let hlc = ark_store::sync_versions::bump_sync_version_vector(
            self.transaction,
            kind.as_str(),
            id,
            device_id,
            true,
        )?;
        ark_store::sync_tombstones::record(self.transaction, kind.as_str(), id, type_id, &hlc)?;
        Ok(hlc)
    }

    fn write_object_sync_version(
        &self,
        id: &str,
        hlc: &str,
        deleted: bool,
    ) -> Result<usize, String> {
        ark_store::sync_versions::write_object_sync_version(self.transaction, id, hlc, deleted)
    }

    fn upsert_game(
        &self,
        command: GameUpsertCommand,
        fallback_device_id: &str,
    ) -> Result<GameMutationResult, String> {
        crate::canonical_types::game::upsert_game(self.transaction, command, fallback_device_id)
    }
}
