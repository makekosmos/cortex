use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObjectWrite, SyncEntity};
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
pub struct ObjectDeleteCommit {
    pub entity: SyncEntity,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMutationError {
    UnsupportedType,
    StaleSnapshot,
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

pub fn validate_local_object_upsert(
    write: &LocalObjectUpsert,
    current_snapshot: Option<&ObjectWriteSnapshot>,
) -> Result<(), ObjectMutationError> {
    if write.object.type_id == "core.task" {
        return Err(ObjectMutationError::UnsupportedType);
    }
    if write
        .expected_snapshot
        .as_ref()
        .is_some_and(|expected| Some(expected) != current_snapshot)
    {
        return Err(ObjectMutationError::StaleSnapshot);
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
