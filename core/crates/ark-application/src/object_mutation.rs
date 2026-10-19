use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ArkObjectWrite, SyncEntity};
use serde_json::json;
use std::fmt;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalObjectUpsert {
    pub object: ArkObjectWrite,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalObjectDelete {
    pub id: String,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectUpsertCommit {
    pub entity: SyncEntity,
    pub id: String,
    pub type_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectUpsertOutcome {
    pub value: ObjectUpsertCommit,
    pub replayed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectDeleteCommit {
    pub entity: SyncEntity,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMutationError {
    UnsupportedType,
    StaleSnapshot,
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectRevisionKind {
    Object,
    ObjectLink,
}

impl ObjectRevisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::ObjectLink => "object_link",
        }
    }
}

pub trait ObjectDeleteSession {
    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String>;
    fn object_type_id(&self, id: &str) -> Result<Option<String>, String>;
    fn delete_object(&self, id: &str) -> Result<(), String>;
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
}

#[derive(Debug, PartialEq, Eq)]
pub enum ObjectDeleteError {
    Storage(String),
    StaleSnapshot,
    StaleRevision,
}

impl fmt::Display for ObjectDeleteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => formatter.write_str(error),
            Self::StaleSnapshot => formatter.write_str("object_conflict:stale_snapshot"),
            Self::StaleRevision => formatter.write_str("object_conflict:stale_revision"),
        }
    }
}

impl std::error::Error for ObjectDeleteError {}

pub fn apply_object_delete(
    session: &dyn ObjectDeleteSession,
    write: LocalObjectDelete,
) -> Result<ObjectDeleteCommit, ObjectDeleteError> {
    if let Some(expected) = &write.expected_snapshot {
        if session
            .object_snapshot(&write.id)
            .map_err(ObjectDeleteError::Storage)?
            != *expected
        {
            return Err(ObjectDeleteError::StaleSnapshot);
        }
    }
    let type_id = session
        .object_type_id(&write.id)
        .map_err(ObjectDeleteError::Storage)?;
    session
        .delete_object(&write.id)
        .map_err(ObjectDeleteError::Storage)?;
    let hlc = session
        .record_delete(
            ObjectRevisionKind::Object,
            &write.id,
            &write.device_id,
            type_id.as_deref(),
        )
        .map_err(ObjectDeleteError::Storage)?;
    if session
        .write_object_sync_version(&write.id, &hlc, true)
        .map_err(ObjectDeleteError::Storage)?
        != 1
    {
        return Err(ObjectDeleteError::StaleRevision);
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
    data: serde_json::Value,
    hlc: String,
    deleted: Option<bool>,
) -> SyncEntity {
    let mut data = match data {
        serde_json::Value::Object(data) => data,
        _ => Default::default(),
    };
    data.remove("id");
    SyncEntity {
        entity_type: entity_type.to_owned(),
        id: id.to_owned(),
        data,
        hlc,
        deleted,
        origin_device_id: None,
        origin_seq: None,
    }
}

impl fmt::Display for ObjectMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedType => "object_write_slice_unsupported_type",
            Self::StaleSnapshot => "object_conflict:stale_snapshot",
        })
    }
}

impl std::error::Error for ObjectMutationError {}

pub fn build_local_object_upsert_commit(object: ArkObject, hlc: String) -> ObjectUpsertCommit {
    let id = object.id.clone();
    let type_id = object.type_id.clone();
    let mut data = serde_json::to_value(&object)
        .unwrap_or_else(|_| json!({}))
        .as_object()
        .cloned()
        .unwrap_or_default();
    data.remove("id");

    ObjectUpsertCommit {
        entity: SyncEntity {
            entity_type: "object".to_owned(),
            id: id.clone(),
            data,
            hlc,
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        },
        id,
        type_id,
    }
}

pub fn validate_local_object_upsert(
    write: &LocalObjectUpsert,
    current_snapshot: Option<&ObjectWriteSnapshot>,
) -> Result<(), ObjectMutationError> {
    validate_local_object_upsert_type(write)?;
    if write
        .expected_snapshot
        .as_ref()
        .is_some_and(|expected| Some(expected) != current_snapshot)
    {
        return Err(ObjectMutationError::StaleSnapshot);
    }
    Ok(())
}

pub fn validate_local_object_upsert_type(
    write: &LocalObjectUpsert,
) -> Result<(), ObjectMutationError> {
    if write.object.type_id == "core.task" {
        return Err(ObjectMutationError::UnsupportedType);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_local_object_upsert, LocalObjectUpsert, ObjectMutationError};
    use ark_domain::object_write::ObjectWriteSnapshot;
    use ark_domain::types::ArkObjectWrite;
    use serde_json::json;

    fn write(type_id: &str) -> LocalObjectUpsert {
        LocalObjectUpsert {
            object: ArkObjectWrite {
                id: "object-1".into(),
                type_id: type_id.into(),
                type_version: None,
                title: "Object".into(),
                content_json: json!({}),
                props_json: json!({}),
                created_at: "2026-10-19T00:00:00Z".into(),
                updated_at: "2026-10-19T00:00:00Z".into(),
                deleted_at: None,
            },
            expected_snapshot: None,
            device_id: "device-1".into(),
        }
    }

    #[test]
    fn local_upsert_refuses_core_task_before_storage_work() {
        assert_eq!(
            validate_local_object_upsert(&write("core.task"), None),
            Err(ObjectMutationError::UnsupportedType)
        );
    }

    #[test]
    fn local_upsert_refuses_a_stale_expected_snapshot() {
        let mut write = write("note");
        write.expected_snapshot = Some(ObjectWriteSnapshot {
            exists: true,
            type_id: Some("note".into()),
            type_version: Some("1".into()),
            revision: Some("revision-1".into()),
        });
        let current = ObjectWriteSnapshot {
            exists: true,
            type_id: Some("note".into()),
            type_version: Some("1".into()),
            revision: Some("revision-2".into()),
        };

        assert_eq!(
            validate_local_object_upsert(&write, Some(&current)),
            Err(ObjectMutationError::StaleSnapshot)
        );
    }
}
