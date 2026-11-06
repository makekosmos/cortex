use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ObjectLink, SyncEntity};
use rusqlite::Connection;

pub struct SqliteTaskMutationSession<'connection> {
    connection: &'connection Connection,
    object_write: crate::object_write::SqliteObjectWriteSession<'connection>,
}

impl<'connection> SqliteTaskMutationSession<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self {
            connection,
            object_write: crate::object_write::SqliteObjectWriteSession::new(connection),
        }
    }
}

impl SqliteTaskMutationSession<'_> {
    pub(crate) fn current_revision(&self, id: &str) -> Result<Option<String>, String> {
        self.object_write.current_revision(id)
    }

    pub(crate) fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        self.object_write.record_remote_revision(id, source_hlc)
    }

    pub(crate) fn persist_task(
        &self,
        object: &ArkObject,
        links: &[ObjectLink],
    ) -> Result<(), String> {
        self.object_write.persist_object(object, links)
    }

    pub(crate) fn task_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        self.object_write.object_snapshot(id)
    }

    pub(crate) fn task_type_id(&self, id: &str) -> Result<Option<String>, String> {
        crate::objects::get_object(self.connection, id)
            .map(|object| object.map(|object| object.type_id))
    }

    pub(crate) fn record_canonical_task_revision(
        &self,
        id: &str,
        source_hlc: &str,
    ) -> Result<(), String> {
        crate::sync_versions::record_remote_object_revision(self.connection, id, source_hlc, false)
    }

    pub(crate) fn hard_delete_remote_task(&self, entity: &SyncEntity) -> Result<(), String> {
        crate::objects::delete_object(self.connection, &entity.id)?;
        crate::sync_versions::record_remote_object_deletion(self.connection, entity)
    }

    pub(crate) fn commit_local_task_upsert(
        &self,
        object: &ArkObject,
        links: &[ObjectLink],
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::task_merge::commit_local_task_upsert(
            self.connection,
            object,
            links,
            device_id,
            expected_snapshot,
        )
    }

    pub(crate) fn commit_local_task_delete(
        &self,
        id: &str,
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
        hard_delete: bool,
    ) -> Result<String, String> {
        crate::task_merge::commit_local_task_delete(
            self.connection,
            id,
            device_id,
            expected_snapshot,
            hard_delete,
        )
    }
}

use ark_domain::hlc::HLC;
use serde::Serialize;
use serde_json::json;
pub enum InboundTaskMutation<'a> {
    CanonicalFullSnapshot {
        object: &'a ArkObject,
        links: &'a [ObjectLink],
        source_hlc: &'a str,
    },
    CanonicalEntitySnapshot {
        object: &'a ArkObject,
        links: &'a [ObjectLink],
        source_hlc: &'a str,
    },
    RemoteHardDelete {
        entity: &'a SyncEntity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundTaskMutationOutcome {
    Applied,
    Stale,
}

pub fn apply_inbound_task_mutation(
    session: &SqliteTaskMutationSession<'_>,
    mutation: InboundTaskMutation<'_>,
) -> Result<InboundTaskMutationOutcome, String> {
    match mutation {
        InboundTaskMutation::CanonicalFullSnapshot {
            object,
            links,
            source_hlc,
        } => {
            validate_task(object)?;
            session.persist_task(object, links)?;
            session.record_remote_revision(&object.id, source_hlc)?;
        }
        InboundTaskMutation::CanonicalEntitySnapshot {
            object,
            links,
            source_hlc,
        } => {
            validate_task(object)?;
            if session
                .current_revision(&object.id)?
                .as_deref()
                .is_some_and(|current| {
                    HLC::compare_str(source_hlc, current) != std::cmp::Ordering::Greater
                })
            {
                return Ok(InboundTaskMutationOutcome::Stale);
            }
            session.persist_task(object, links)?;
            session.record_canonical_task_revision(&object.id, source_hlc)?;
        }
        InboundTaskMutation::RemoteHardDelete { entity } => {
            if session
                .current_revision(&entity.id)?
                .as_deref()
                .is_some_and(|current| {
                    HLC::compare_str(&entity.hlc, current) != std::cmp::Ordering::Greater
                })
            {
                return Ok(InboundTaskMutationOutcome::Stale);
            }
            session.hard_delete_remote_task(entity)?;
        }
    }
    Ok(InboundTaskMutationOutcome::Applied)
}

fn validate_task(object: &ArkObject) -> Result<(), String> {
    if object.type_id != "core.task"
        || !ark_domain::canonical_type::definitions::is_supported_core_task_version(
            &object.type_version,
        )
    {
        return Err("object_write_slice_unsupported_type".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalTaskDelete {
    pub id: String,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
    pub mode: TaskDeleteMode,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TaskDeleteMode {
    Hard,
    Soft,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskDeleteCommit {
    pub entity: SyncEntity,
    pub id: String,
}

pub fn apply_task_upsert(
    session: &SqliteTaskMutationSession<'_>,
    object: &ArkObject,
    links: &[ObjectLink],
    device_id: &str,
    expected_snapshot: Option<&ObjectWriteSnapshot>,
) -> Result<Vec<SyncEntity>, String> {
    if object.type_id != "core.task"
        || !ark_domain::canonical_type::definitions::is_supported_core_task_version(
            &object.type_version,
        )
    {
        return Err("object_write_slice_unsupported_type".into());
    }
    session.commit_local_task_upsert(object, links, device_id, expected_snapshot)
}

pub fn apply_task_delete(
    session: &SqliteTaskMutationSession<'_>,
    write: LocalTaskDelete,
) -> Result<TaskDeleteCommit, String> {
    let type_id = session.task_type_id(&write.id)?;
    if type_id.as_deref() != Some("core.task") {
        return Err(if type_id.is_some() {
            "WRONG_TARGET"
        } else {
            "MISSING_OBJECT"
        }
        .into());
    }
    if let Some(expected) = &write.expected_snapshot {
        if session.task_snapshot(&write.id)? != *expected {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    let hlc = session.commit_local_task_delete(
        &write.id,
        &write.device_id,
        write.expected_snapshot.as_ref(),
        matches!(write.mode, TaskDeleteMode::Hard),
    )?;
    let data = json!({ "typeId": "core.task" });
    Ok(TaskDeleteCommit {
        entity: SyncEntity {
            entity_type: "object".into(),
            id: write.id.clone(),
            data: data.as_object().cloned().unwrap_or_default(),
            hlc,
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        },
        id: write.id,
    })
}
