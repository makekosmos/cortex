use crate::canonical_types::facades::BookCoverMutation;
use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::domain::object_write::ObjectWriteSnapshot;
use crate::types::{ArkObject, ArkObjectWrite, SyncEntity};
use serde::Serialize;
use serde_json::{json, Map, Value};

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
pub struct LocalObjectUpsert {
    pub object: ArkObjectWrite,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalObjectDelete {
    pub id: String,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalBookCoverWrite {
    pub book_id: String,
    pub source_ref: Option<String>,
    pub existing_image_id: Option<String>,
    pub alt_text: String,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub struct ObjectUpsertCommit {
    pub entity: SyncEntity,
    pub id: String,
    pub type_id: String,
}

#[derive(Debug, Clone)]
pub struct ObjectDeleteCommit {
    pub entity: SyncEntity,
    pub id: String,
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
    if write.object.type_id == "core.task" {
        return Err("object_write_slice_unsupported_type".into());
    }
    if let Some(expected) = &write.expected_snapshot {
        if session.object_snapshot(&write.object.id)? != *expected {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    let object = session.prepare_object(write.object)?;
    session.persist_object(&object)?;
    let hlc = session.record_upsert(ObjectRevisionKind::Object, &object.id, &write.device_id)?;
    if session.write_object_sync_version(&object.id, &hlc, false)? != 1 {
        return Err("object_conflict:stale_revision".into());
    }
    let entity = object_sync_entity(&object.id, &object, hlc, None);
    Ok(ObjectUpsertCommit {
        entity,
        id: object.id,
        type_id: object.type_id,
    })
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

fn object_sync_entity(
    id: &str,
    object: &ArkObject,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let data = serde_json::to_value(object)
        .unwrap_or_else(|_| json!({}))
        .as_object()
        .cloned()
        .unwrap_or_default();
    make_sync_entity("object", id, Value::Object(data), hlc, deleted)
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
