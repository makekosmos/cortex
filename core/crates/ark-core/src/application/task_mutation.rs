use crate::domain::object_write::ObjectWriteSnapshot;
use crate::types::{ArkObject, ObjectLink, SyncEntity};
use serde::Serialize;
use serde_json::json;

pub(crate) trait TaskMutationSession:
    crate::application::objects::ObjectWriteSession
{
    fn persist_task(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String>;
    fn record_task_upsert(&self, id: &str, device_id: &str) -> Result<String, String>;
    fn record_task_link_upsert(&self, id: &str, device_id: &str) -> Result<String, String>;
    fn task_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String>;
    fn task_type_id(&self, id: &str) -> Result<Option<String>, String>;
    fn hard_delete_task(&self, id: &str, device_id: &str) -> Result<String, String>;
    fn soft_delete_task(&self, id: &str, device_id: &str) -> Result<String, String>;
    fn legacy_task_revision(&self, id: &str) -> Result<Option<String>, String>;
    fn record_legacy_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn record_canonical_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn hard_delete_remote_task(&self, entity: &SyncEntity) -> Result<(), String>;
}

pub(crate) enum InboundTaskMutation<'a> {
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
pub(crate) enum InboundTaskMutationOutcome {
    Applied,
    Stale,
}

pub(crate) fn apply_inbound_task_mutation(
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
                    crate::hlc::HLC::compare_str(source_hlc, current) != std::cmp::Ordering::Greater
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
            if session
                .legacy_task_revision(&object.id)?
                .as_deref()
                .is_some_and(|current| {
                    crate::hlc::HLC::compare_str(source_hlc, current) != std::cmp::Ordering::Greater
                })
            {
                return Ok(InboundTaskMutationOutcome::Stale);
            }
            session.persist_task(object, links)?;
            session.record_legacy_task_revision(&object.id, source_hlc)?;
        }
        InboundTaskMutation::RemoteHardDelete { entity } => {
            if session
                .current_revision(&entity.id)?
                .as_deref()
                .is_some_and(|current| {
                    crate::hlc::HLC::compare_str(&entity.hlc, current)
                        != std::cmp::Ordering::Greater
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
pub(crate) struct LocalTaskDelete {
    pub id: String,
    pub expected_snapshot: Option<ObjectWriteSnapshot>,
    pub device_id: String,
    pub mode: TaskDeleteMode,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) enum TaskDeleteMode {
    Hard,
    Soft,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TaskDeleteCommit {
    pub entity: SyncEntity,
    pub id: String,
}

pub(crate) fn apply_task_upsert(
    session: &dyn TaskMutationSession,
    object: &ArkObject,
    links: &[ObjectLink],
    device_id: &str,
) -> Result<Vec<SyncEntity>, String> {
    if object.type_id != "core.task" || object.type_version != "0.1.0" {
        return Err("object_write_slice_unsupported_type".into());
    }
    session.persist_task(object, links)?;
    let hlc = session.record_task_upsert(&object.id, device_id)?;
    let mut entities = vec![sync_entity("object", &object.id, object, &hlc, None)?];
    for link in links {
        let hlc = session.record_task_link_upsert(&link.id, device_id)?;
        entities.push(sync_entity("object_link", &link.id, link, &hlc, None)?);
    }
    Ok(entities)
}

pub(crate) fn apply_task_delete(
    session: &dyn TaskMutationSession,
    write: LocalTaskDelete,
) -> Result<TaskDeleteCommit, String> {
    if let Some(expected) = &write.expected_snapshot {
        if session.task_snapshot(&write.id)? != *expected {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    let type_id = session.task_type_id(&write.id)?;
    if type_id.as_deref() != Some("core.task") {
        return Err(if type_id.is_some() {
            "WRONG_TARGET"
        } else {
            "MISSING_OBJECT"
        }
        .into());
    }
    let hlc = match write.mode {
        TaskDeleteMode::Hard => session.hard_delete_task(&write.id, &write.device_id)?,
        TaskDeleteMode::Soft => session.soft_delete_task(&write.id, &write.device_id)?,
    };
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

fn sync_entity<T: serde::Serialize>(
    entity_type: &str,
    id: &str,
    value: &T,
    hlc: &str,
    deleted: Option<bool>,
) -> Result<SyncEntity, String> {
    let data = serde_json::to_value(value)
        .map_err(|error| error.to_string())?
        .as_object()
        .cloned()
        .unwrap_or_default();
    Ok(SyncEntity {
        entity_type: entity_type.into(),
        id: id.into(),
        data,
        hlc: hlc.into(),
        deleted,
        origin_device_id: None,
        origin_seq: None,
    })
}
