use crate::object_link_mutation::SqliteObjectLinkMutationSession;
use crate::type_registry::mutation_session::SqliteTypeMutationSession;
use domain::r#type::TypeRegistration;
use domain::sync_storage::ReplayNotice;
use domain::types::{ObjectLink, SyncEntity};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy)]
pub enum TypeEntityKind {
    ObjectType,
    ObjectLink,
}

impl TypeEntityKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ObjectType => "object_type",
            Self::ObjectLink => "object_link",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeMutationCommit {
    pub entities: Vec<SyncEntity>,
    pub replay_notices: Vec<ReplayNotice>,
}

#[derive(Debug, Clone)]
pub struct ReplaceObjectLinkCommit {
    pub object_link: ObjectLink,
    pub mutation: TypeMutationCommit,
}

pub fn register_application_type(
    session: &SqliteTypeMutationSession<'_, '_>,
    registration: &TypeRegistration,
) -> Result<Vec<ReplayNotice>, String> {
    session.register_application_type(registration)
}

pub fn upsert_object_link(
    session: &SqliteObjectLinkMutationSession<'_, '_>,
    object_link: ObjectLink,
    device_id: Option<&str>,
) -> Result<TypeMutationCommit, String> {
    session.upsert_object_link(&object_link)?;
    let hlc = session.record_local_revision(&object_link.id, device_id, false)?;
    let entity = sync_entity(
        TypeEntityKind::ObjectLink,
        &object_link.id,
        serde_json::to_value(&object_link).unwrap_or_else(|_| serde_json::json!({})),
        hlc,
        None,
    );
    Ok(TypeMutationCommit {
        entities: vec![entity],
        replay_notices: Vec::new(),
    })
}

pub fn delete_object_link(
    session: &SqliteObjectLinkMutationSession<'_, '_>,
    id: String,
    device_id: Option<&str>,
) -> Result<TypeMutationCommit, String> {
    session.delete_object_link(&id)?;
    let hlc = session.record_local_revision(&id, device_id, true)?;
    Ok(TypeMutationCommit {
        entities: vec![sync_entity(
            TypeEntityKind::ObjectLink,
            &id,
            serde_json::json!({}),
            hlc,
            Some(true),
        )],
        replay_notices: Vec::new(),
    })
}

pub fn replace_object_link(
    session: &SqliteObjectLinkMutationSession<'_, '_>,
    object_link: ObjectLink,
    device_id: Option<&str>,
) -> Result<ReplaceObjectLinkCommit, String> {
    let mut entities = Vec::new();
    for id in session.other_object_link_ids(
        &object_link.source_object_id,
        &object_link.link_type,
        &object_link.id,
    )? {
        session.delete_object_link(&id)?;
        let hlc = session.record_local_revision(&id, device_id, true)?;
        entities.push(sync_entity(
            TypeEntityKind::ObjectLink,
            &id,
            serde_json::json!({}),
            hlc,
            Some(true),
        ));
    }
    session.upsert_object_link(&object_link)?;
    let hlc = session.record_local_revision(&object_link.id, device_id, false)?;
    entities.push(sync_entity(
        TypeEntityKind::ObjectLink,
        &object_link.id,
        serde_json::to_value(&object_link).unwrap_or_else(|_| serde_json::json!({})),
        hlc,
        None,
    ));
    Ok(ReplaceObjectLinkCommit {
        object_link,
        mutation: TypeMutationCommit {
            entities,
            replay_notices: Vec::new(),
        },
    })
}

fn sync_entity(
    kind: TypeEntityKind,
    id: &str,
    data: Value,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let data = match data {
        Value::Object(mut fields) => {
            fields.remove("id");
            fields
        }
        _ => Map::new(),
    };
    SyncEntity {
        entity_type: kind.as_str().to_owned(),
        id: id.to_owned(),
        data,
        hlc,
        deleted,
        origin_device_id: None,
        origin_seq: None,
    }
}
