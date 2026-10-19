use crate::canonical_types::facades::BookCoverMutation;
use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::types::{ArkObject, ArkObjectWrite, SyncEntity};
use ark_domain::object_write::ObjectWriteSnapshot;
use serde::Serialize;
use serde_json::{json, Map, Value};

pub use ark_application::object_mutation::{
    LocalObjectDelete, LocalObjectUpsert, ObjectDeleteCommit, ObjectUpsertCommit,
};

#[derive(Debug, Clone, Copy)]
pub enum ObjectRevisionKind {
    Object,
    ObjectLink,
}

impl ObjectRevisionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::ObjectLink => "object_link",
        }
    }
}

pub trait ObjectMutationSession {
    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String>;
    fn prepare_object(&self, write: ArkObjectWrite) -> Result<ArkObject, String>;
    fn persist_object(&self, object: &ArkObject) -> Result<(), String>;
    fn object_type_id(&self, id: &str) -> Result<Option<String>, String>;
    fn delete_object(&self, id: &str) -> Result<(), String>;
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

pub fn apply_object_upsert(
    session: &dyn ObjectMutationSession,
    write: LocalObjectUpsert,
) -> Result<ObjectUpsertCommit, String> {
    ark_application::object_mutation::validate_local_object_upsert_type(&write)
        .map_err(|error| error.to_string())?;
    let current_snapshot = write
        .expected_snapshot
        .as_ref()
        .map(|_| session.object_snapshot(&write.object.id))
        .transpose()?;
    ark_application::object_mutation::validate_local_object_upsert(
        &write,
        current_snapshot.as_ref(),
    )
    .map_err(|error| error.to_string())?;
    let object = session.prepare_object(write.object)?;
    session.persist_object(&object)?;
    let hlc = session.record_upsert(ObjectRevisionKind::Object, &object.id, &write.device_id)?;
    Ok(ark_application::object_mutation::build_local_object_upsert_commit(object, hlc))
}

pub fn apply_object_delete(
    session: &dyn ObjectMutationSession,
    write: LocalObjectDelete,
) -> Result<ObjectDeleteCommit, String> {
    if let Some(expected) = &write.expected_snapshot {
        if session.object_snapshot(&write.id)? != *expected {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    let type_id = session.object_type_id(&write.id)?;
    session.delete_object(&write.id)?;
    let hlc = session.record_delete(
        ObjectRevisionKind::Object,
        &write.id,
        &write.device_id,
        type_id.as_deref(),
    )?;
    if session.write_object_sync_version(&write.id, &hlc, true)? != 1 {
        return Err("object_conflict:stale_revision".into());
    }
    let data = type_id
        .map(|type_id| json!({ "typeId": type_id }))
        .unwrap_or_else(|| json!({}));
    Ok(ObjectDeleteCommit {
        entity: make_sync_entity("object", &write.id, data, hlc, Some(true)),
        id: write.id,
    })
}

fn make_sync_entity(
    entity_type: &str,
    id: &str,
    data: Value,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let mut data = match data {
        Value::Object(data) => data,
        _ => Map::new(),
    };
    data.remove("id");
    SyncEntity {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        data,
        hlc,
        deleted,
        origin_device_id: None,
        origin_seq: None,
    }
}
