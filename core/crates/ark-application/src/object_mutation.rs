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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalObjectDeleteFacts {
    pub current_snapshot: ObjectWriteSnapshot,
    pub type_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalObjectDeletePlan {
    pub id: String,
    pub device_id: String,
    pub type_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMutationError {
    UnsupportedType,
    StaleSnapshot,
    StaleRevision,
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

impl fmt::Display for ObjectMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedType => "object_write_slice_unsupported_type",
            Self::StaleSnapshot => "object_conflict:stale_snapshot",
            Self::StaleRevision => "object_conflict:stale_revision",
        })
    }
}

#[derive(Debug)]
pub enum ObjectDeleteError {
    Storage(String),
    Conflict(ObjectMutationError),
    TaskMutation(String),
    OperationIdConflict,
}

impl fmt::Display for ObjectDeleteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) | Self::TaskMutation(error) => formatter.write_str(error),
            Self::Conflict(error) => fmt::Display::fmt(error, formatter),
            Self::OperationIdConflict => {
                formatter.write_str("operationId was already used with a different command payload")
            }
        }
    }
}

impl std::error::Error for ObjectDeleteError {}

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

pub fn plan_local_object_delete(
    write: LocalObjectDelete,
    facts: LocalObjectDeleteFacts,
) -> Result<LocalObjectDeletePlan, ObjectMutationError> {
    if write
        .expected_snapshot
        .as_ref()
        .is_some_and(|expected| expected != &facts.current_snapshot)
    {
        return Err(ObjectMutationError::StaleSnapshot);
    }
    Ok(LocalObjectDeletePlan {
        id: write.id,
        device_id: write.device_id,
        type_id: facts.type_id,
    })
}

pub fn build_local_object_delete_commit(
    plan: LocalObjectDeletePlan,
    hlc: String,
) -> ObjectDeleteCommit {
    let mut data = serde_json::Map::new();
    if let Some(type_id) = plan.type_id {
        data.insert("typeId".into(), type_id.into());
    }
    ObjectDeleteCommit {
        entity: SyncEntity {
            entity_type: "object".into(),
            id: plan.id.clone(),
            data,
            hlc,
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        },
        id: plan.id,
    }
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

#[cfg(test)]
mod local_object_delete_tests {
    use super::{
        build_local_object_delete_commit, plan_local_object_delete, LocalObjectDelete,
        LocalObjectDeleteFacts, ObjectMutationError,
    };
    use ark_domain::object_write::ObjectWriteSnapshot;

    fn snapshot(revision: &str) -> ObjectWriteSnapshot {
        ObjectWriteSnapshot {
            exists: true,
            type_id: Some("com.kosmos.note".into()),
            type_version: Some("1.0.0".into()),
            revision: Some(revision.into()),
        }
    }

    #[test]
    fn delete_plan_requires_the_expected_snapshot_and_builds_sync_commit() {
        let write = LocalObjectDelete {
            id: "note-1".into(),
            expected_snapshot: Some(snapshot("revision-1")),
            device_id: "device-1".into(),
        };
        let facts = LocalObjectDeleteFacts {
            current_snapshot: snapshot("revision-2"),
            type_id: Some("com.kosmos.note".into()),
        };
        assert_eq!(
            plan_local_object_delete(write.clone(), facts),
            Err(ObjectMutationError::StaleSnapshot)
        );

        let plan = plan_local_object_delete(
            write,
            LocalObjectDeleteFacts {
                current_snapshot: snapshot("revision-1"),
                type_id: Some("com.kosmos.note".into()),
            },
        )
        .expect("current snapshot creates a delete plan");
        let commit = build_local_object_delete_commit(plan, "time:1:device-1".into());
        assert_eq!(commit.id, "note-1");
        assert_eq!(commit.entity.data["typeId"], "com.kosmos.note");
        assert_eq!(commit.entity.deleted, Some(true));
    }
}
