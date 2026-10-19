use crate::application::object_mutation::{ObjectMutationSession, ObjectRevisionKind};
use crate::canonical_types::facades::BookCoverMutation;
use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::db;
use crate::types::ArkObject;
use ark_domain::object_write::ObjectWriteSnapshot;
use rusqlite::{params, OptionalExtension, Transaction};

pub(crate) struct SqliteObjectMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteObjectMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl ObjectMutationSession for SqliteObjectMutationSession<'_, '_> {
    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        let identity = self
            .transaction
            .query_row(
                "SELECT type_id, type_version FROM objects WHERE id = ?1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        Ok(ObjectWriteSnapshot {
            exists: identity.is_some(),
            type_id: identity.as_ref().map(|value| value.0.clone()),
            type_version: identity.map(|value| value.1),
            revision: ark_store::sync_versions::object_revision(self.transaction, id)?,
        })
    }

    fn persist_object(&self, object: &ArkObject) -> Result<(), String> {
        db::upsert_object(self.transaction, object)
    }

    fn object_type_id(&self, id: &str) -> Result<Option<String>, String> {
        Ok(db::get_object(self.transaction, id)?.map(|object| object.type_id))
    }

    fn delete_object(&self, id: &str) -> Result<(), String> {
        db::delete_object(self.transaction, id)
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
        self.transaction
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, ?3)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = excluded.deleted
                 WHERE excluded.hlc > object_sync_versions.hlc",
                params![id, hlc, deleted],
            )
            .map_err(|error| error.to_string())
    }

    fn upsert_game(
        &self,
        command: GameUpsertCommand,
        fallback_device_id: &str,
    ) -> Result<GameMutationResult, String> {
        crate::canonical_types::game::upsert_game(self.transaction, command, fallback_device_id)
    }

    fn set_book_cover(
        &self,
        book_id: &str,
        source_ref: Option<&str>,
        existing_image_id: Option<&str>,
        alt_text: &str,
        device_id: &str,
    ) -> Result<BookCoverMutation, String> {
        crate::canonical_types::facades::set_book_cover(
            self.transaction,
            book_id,
            source_ref,
            existing_image_id,
            alt_text,
            device_id,
        )
        .map_err(|error| error.to_string())
    }
}
