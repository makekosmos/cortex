use crate::application::task_mutation::TaskMutationSession;
use crate::hlc::HLC;
use crate::types::SyncEntity;
use rusqlite::Connection;

/// Canonical ingress for compatibility Todo/Project/Tag writes.  Adapters may
/// construct the legacy wire record, but persistence, HLC and link state stay
/// owned by this facade.
pub fn write_legacy_records(
    conn: &Connection,
    records: &[ark_application::canonical_compatibility::LegacyRecord],
    source_kind: &str,
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    use ark_application::canonical_compatibility::{
        map_legacy_with_context, CanonicalIdentity, LegacySource, MappingContext,
    };
    use std::collections::BTreeMap;
    let mut existing_object_ids = BTreeMap::new();
    for object in ark_store::objects::list_objects(conn)? {
        existing_object_ids.insert(
            object.id,
            CanonicalIdentity::new(object.type_id, object.type_version),
        );
    }
    for record in records {
        let type_id = match record.legacy_type_id.as_str() {
            "task_obj" => "core.task",
            "project_obj" => "com.kosmos.project",
            "tag_obj" => "com.kosmos.tag",
            _ => continue,
        };
        let version = crate::canonical_types::definitions::current_canonical_version(type_id)
            .ok()
            .flatten()
            .unwrap_or_default();
        existing_object_ids.insert(record.id.clone(), CanonicalIdentity::new(type_id, version));
    }
    let context = MappingContext {
        existing_object_ids,
        existing_links: ark_store::objects::list_object_links(conn)?,
    };
    let mapped = records
        .iter()
        .map(|record| {
            let raw = serde_json::to_vec(record).map_err(|e| e.to_string())?;
            map_legacy_with_context(
                LegacySource {
                    source_kind,
                    source_id: &record.id,
                    raw_json: &raw,
                },
                &context,
            )
            .map_err(|e| format!("{}:{}:{}", e.code(), e.source_id(), e.pointer()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let write_device = device_id.as_deref().unwrap_or("ark-core");
    let mut entities = Vec::new();
    for item in mapped {
        let id = item.object.id.clone();
        if item.object.type_id == "core.task" {
            let session = ark_store::task_merge::SqliteTaskMutationSession::new(conn);
            let expected_snapshot = session.task_snapshot(&item.object.id)?;
            entities.extend(crate::application::task_mutation::apply_task_upsert(
                &session,
                &item.object,
                &item.links,
                write_device,
                Some(&expected_snapshot),
            )?);
            continue;
        }
        ark_store::objects::upsert_object_compat(conn, &item.object)?;
        let hlc = ark_store::sync_versions::bump_sync_version_vector(
            conn,
            "object",
            &id,
            write_device,
            false,
        )?;
        ark_store::sync_versions::write_object_sync_version(conn, &id, &hlc, false)?;
        entities.push(SyncEntity {
            entity_type: "object".into(),
            id: id.clone(),
            data: serde_json::to_value(&item.object)
                .map_err(|e| e.to_string())?
                .as_object()
                .cloned()
                .unwrap_or_default(),
            hlc,
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        });
        for link in item.links {
            let link_id = link.id.clone();
            ark_store::objects::upsert_object_link(conn, &link)?;
            let hlc = ark_store::sync_versions::bump_sync_version_vector(
                conn,
                "object_link",
                &link_id,
                write_device,
                false,
            )?;
            entities.push(SyncEntity {
                entity_type: "object_link".into(),
                id: link_id,
                data: serde_json::to_value(&link)
                    .map_err(|e| e.to_string())?
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                hlc,
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            });
        }
    }
    Ok(entities)
}

pub fn delete_legacy_object(
    conn: &Connection,
    id: &str,
    expected_type_id: &str,
    device_id: Option<String>,
) -> Result<SyncEntity, String> {
    if expected_type_id == "core.task" {
        let session = ark_store::task_merge::SqliteTaskMutationSession::new(conn);
        let commit = crate::application::task_mutation::apply_task_delete(
            &session,
            crate::application::task_mutation::LocalTaskDelete {
                id: id.into(),
                expected_snapshot: None,
                device_id: device_id.as_deref().unwrap_or("ark-core").into(),
                mode: crate::application::task_mutation::TaskDeleteMode::Soft,
            },
        )?;
        return Ok(commit.entity);
    }
    let mut object =
        ark_store::objects::get_object(conn, id)?.ok_or_else(|| "MISSING_OBJECT".to_string())?;
    if object.type_id != expected_type_id {
        return Err("WRONG_TARGET".into());
    }
    let write_device = device_id.as_deref().unwrap_or("ark-core");
    object.deleted_at = Some(HLC::now(write_device).wall_time);
    ark_store::objects::upsert_object_compat(conn, &object)?;
    let hlc =
        ark_store::sync_versions::bump_sync_version_vector(conn, "object", id, write_device, true)?;
    ark_store::sync_versions::write_object_sync_version(conn, id, &hlc, true)?;
    ark_store::sync_tombstones::record(conn, "object", id, Some(&object.type_id), &hlc)?;
    Ok(SyncEntity {
        entity_type: "object".into(),
        id: id.into(),
        data: [("typeId".into(), object.type_id.into())]
            .into_iter()
            .collect(),
        hlc,
        deleted: Some(true),
        origin_device_id: None,
        origin_seq: None,
    })
}
