use crate::canonical_types::facades::BookCoverMutation;
use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::types::ArkObject;
use serde::Serialize;

pub use ark_application::object_mutation::{
    LocalObjectDelete, LocalObjectUpsert, ObjectDeleteCommit, ObjectRevisionKind,
    ObjectUpsertCommit,
};

pub trait ObjectMutationSession {
    fn persist_object(&self, object: &ArkObject) -> Result<(), String>;
    fn record_upsert(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
    ) -> Result<String, String>;
    fn record_delete(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
        type_id: Option<&str>,
    ) -> Result<String, String>;
    fn write_object_sync_version(
        &self,
        id: &str,
        hlc: &str,
        deleted: bool,
    ) -> Result<usize, String>;
    fn upsert_game(
        &self,
        command: GameUpsertCommand,
        fallback_device_id: &str,
    ) -> Result<GameMutationResult, String>;
    fn set_book_cover(
        &self,
        book_id: &str,
        source_ref: Option<&str>,
        existing_image_id: Option<&str>,
        alt_text: &str,
        device_id: &str,
    ) -> Result<BookCoverMutation, String>;
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalBookCoverWrite {
    pub book_id: String,
    pub source_ref: Option<String>,
    pub existing_image_id: Option<String>,
    pub alt_text: String,
    pub device_id: String,
}

pub fn apply_game_upsert(
    session: &dyn ObjectMutationSession,
    command: GameUpsertCommand,
    fallback_device_id: &str,
) -> Result<GameMutationResult, String> {
    session.upsert_game(command, fallback_device_id)
}

pub fn apply_book_cover(
    session: &dyn ObjectMutationSession,
    write: LocalBookCoverWrite,
) -> Result<bool, String> {
    let mutation = session.set_book_cover(
        &write.book_id,
        write.source_ref.as_deref(),
        write.existing_image_id.as_deref(),
        &write.alt_text,
        &write.device_id,
    )?;
    if !mutation.changed {
        return Ok(false);
    }

    record_book_cover_revisions(session, &write, &mutation)?;
    Ok(true)
}

fn record_book_cover_revisions(
    session: &dyn ObjectMutationSession,
    write: &LocalBookCoverWrite,
    mutation: &BookCoverMutation,
) -> Result<(), String> {
    let book_hlc =
        session.record_upsert(ObjectRevisionKind::Object, &write.book_id, &write.device_id)?;
    session.write_object_sync_version(&write.book_id, &book_hlc, false)?;
    for link in &mutation.links {
        let hlc =
            session.record_upsert(ObjectRevisionKind::ObjectLink, &link.id, &write.device_id)?;
        session.write_object_sync_version(&link.id, &hlc, false)?;
    }
    for id in &mutation.deleted_link_ids {
        session.record_delete(ObjectRevisionKind::ObjectLink, id, &write.device_id, None)?;
    }
    if let Some(image) = &mutation.image {
        session.record_upsert(ObjectRevisionKind::Object, &image.id, &write.device_id)?;
    }
    Ok(())
}
