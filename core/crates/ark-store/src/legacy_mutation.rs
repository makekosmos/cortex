use ark_application::task_mutation::TaskMutationSession;
use ark_domain::hlc::HLC;
use ark_domain::types::SyncEntity;
use rusqlite::Transaction;

pub(crate) fn apply_local_legacy_graph_write_in(
    transaction: &Transaction<'_>,
    records: &[ark_application::canonical_compatibility::LegacyRecord],
    source_kind: &str,
    device_id: Option<&str>,
) -> Result<Vec<SyncEntity>, String> {
    use ark_application::canonical_compatibility::{
        map_legacy_with_context, CanonicalIdentity, LegacySource, MappingContext,
    };
    use std::collections::BTreeMap;

    let mut existing_object_ids = BTreeMap::new();
    for object in crate::objects::list_objects(transaction)? {
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
        let version = ark_domain::canonical_type::definitions::current_canonical_version(type_id)
            .ok()
            .flatten()
            .unwrap_or_default();
        existing_object_ids.insert(record.id.clone(), CanonicalIdentity::new(type_id, version));
    }
    let context = MappingContext {
        existing_object_ids,
        existing_links: crate::objects::list_object_links(transaction)?,
    };
    let mapped = records
        .iter()
        .map(|record| {
            let raw = serde_json::to_vec(record).map_err(|error| error.to_string())?;
            map_legacy_with_context(
                LegacySource {
                    source_kind,
                    source_id: &record.id,
                    raw_json: &raw,
                },
                &context,
            )
            .map_err(|error| format!("{}:{}:{}", error.code(), error.source_id(), error.pointer()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let write_device = device_id.unwrap_or("ark-core");
    let mut entities = Vec::new();
    for item in mapped {
        let id = item.object.id.clone();
        if item.object.type_id == "core.task" {
            let session = crate::task_merge::SqliteTaskMutationSession::new(transaction);
            let expected_snapshot = session.task_snapshot(&item.object.id)?;
            entities.extend(ark_application::task_mutation::apply_task_upsert(
                &session,
                &item.object,
                &item.links,
                write_device,
                Some(&expected_snapshot),
            )?);
            continue;
        }
        crate::objects::upsert_object_compat(transaction, &item.object)?;
        let hlc = crate::sync_versions::bump_sync_version_vector(
            transaction,
            "object",
            &id,
            write_device,
            false,
        )?;
        crate::sync_versions::write_object_sync_version(transaction, &id, &hlc, false)?;
        entities.push(SyncEntity {
            entity_type: "object".into(),
            id: id.clone(),
            data: serde_json::to_value(&item.object)
                .map_err(|error| error.to_string())?
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
            crate::objects::upsert_object_link(transaction, &link)?;
            let hlc = crate::sync_versions::bump_sync_version_vector(
                transaction,
                "object_link",
                &link_id,
                write_device,
                false,
            )?;
            entities.push(SyncEntity {
                entity_type: "object_link".into(),
                id: link_id,
                data: serde_json::to_value(&link)
                    .map_err(|error| error.to_string())?
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

pub(crate) fn apply_local_legacy_delete_in(
    transaction: &Transaction<'_>,
    id: &str,
    expected_type_id: &str,
    device_id: Option<&str>,
) -> Result<SyncEntity, String> {
    if expected_type_id == "core.task" {
        let session = crate::task_merge::SqliteTaskMutationSession::new(transaction);
        let commit = ark_application::task_mutation::apply_task_delete(
            &session,
            ark_application::task_mutation::LocalTaskDelete {
                id: id.into(),
                expected_snapshot: None,
                device_id: device_id.unwrap_or("ark-core").into(),
                mode: ark_application::task_mutation::TaskDeleteMode::Soft,
            },
        )?;
        return Ok(commit.entity);
    }
    let mut object =
        crate::objects::get_object(transaction, id)?.ok_or_else(|| "MISSING_OBJECT".to_string())?;
    if object.type_id != expected_type_id {
        return Err("WRONG_TARGET".into());
    }
    let write_device = device_id.unwrap_or("ark-core");
    object.deleted_at = Some(HLC::now(write_device).wall_time);
    crate::objects::upsert_object_compat(transaction, &object)?;
    let hlc = crate::sync_versions::bump_sync_version_vector(
        transaction,
        "object",
        id,
        write_device,
        true,
    )?;
    crate::sync_versions::write_object_sync_version(transaction, id, &hlc, true)?;
    crate::sync_tombstones::record(transaction, "object", id, Some(&object.type_id), &hlc)?;
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
