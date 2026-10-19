use ark_domain::hlc::HLC;
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ObjectLink, SyncEntity};
use serde::Serialize;
use serde_json::json;

pub trait TaskMutationSession {
    fn current_revision(&self, id: &str) -> Result<Option<String>, String>;
    fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn ensure_legacy_task_write_allowed(&self, id: &str) -> Result<(), String>;
    fn persist_task(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String>;
    fn record_task_upsert(&self, id: &str, device_id: &str) -> Result<String, String>;
    fn record_task_link_upsert(&self, id: &str, device_id: &str) -> Result<String, String>;
    fn task_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String>;
    fn task_type_id(&self, id: &str) -> Result<Option<String>, String>;
    fn legacy_task_revision(&self, id: &str) -> Result<Option<String>, String>;
    fn record_legacy_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn record_canonical_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn hard_delete_remote_task(&self, entity: &SyncEntity) -> Result<(), String>;
    fn commit_local_task_upsert(
        &self,
        object: &ArkObject,
        links: &[ObjectLink],
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
    ) -> Result<Vec<SyncEntity>, String>;
    fn commit_local_task_delete(
        &self,
        id: &str,
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
        hard_delete: bool,
    ) -> Result<String, String>;
}

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
    LegacyTodoSnapshot {
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
    session: &dyn TaskMutationSession,
    mutation: InboundTaskMutation<'_>,
) -> Result<InboundTaskMutationOutcome, String> {
    match mutation {
        InboundTaskMutation::CanonicalFullSnapshot {
            object,
            links,
            source_hlc,
        } => {
            validate_task(object)?;
            session.ensure_legacy_task_write_allowed(&object.id)?;
            session.persist_task(object, links)?;
            session.record_remote_revision(&object.id, source_hlc)?;
        }
        InboundTaskMutation::CanonicalEntitySnapshot {
            object,
            links,
            source_hlc,
        } => {
            validate_task(object)?;
            session.ensure_legacy_task_write_allowed(&object.id)?;
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
        InboundTaskMutation::LegacyTodoSnapshot {
            object,
            links,
            source_hlc,
        } => {
            validate_task(object)?;
            session.ensure_legacy_task_write_allowed(&object.id)?;
            if session
                .legacy_task_revision(&object.id)?
                .as_deref()
                .is_some_and(|current| {
                    HLC::compare_str(source_hlc, current) != std::cmp::Ordering::Greater
                })
            {
                return Ok(InboundTaskMutationOutcome::Stale);
            }
            session.persist_task(object, links)?;
            session.record_legacy_task_revision(&object.id, source_hlc)?;
        }
        InboundTaskMutation::RemoteHardDelete { entity } => {
            session.ensure_legacy_task_write_allowed(&entity.id)?;
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
    if object.type_id != "core.task" || object.type_version != "0.1.0" {
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
    session: &dyn TaskMutationSession,
    object: &ArkObject,
    links: &[ObjectLink],
    device_id: &str,
    expected_snapshot: Option<&ObjectWriteSnapshot>,
) -> Result<Vec<SyncEntity>, String> {
    if object.type_id != "core.task" || object.type_version != "0.1.0" {
        return Err("object_write_slice_unsupported_type".into());
    }
    session.commit_local_task_upsert(object, links, device_id, expected_snapshot)
}

pub fn apply_task_delete(
    session: &dyn TaskMutationSession,
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
