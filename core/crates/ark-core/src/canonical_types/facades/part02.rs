/// Atomically set, replace, or remove the Book's sole canonical cover link.
///
/// This boundary never materializes an Image. `source_ref` is retained in the
/// wire contract for compatibility, but source conversion belongs to the
/// caller/import pipeline; this facade only links an existing typed Image.
pub fn set_book_cover(
    conn: &Connection,
    book_id: &str,
    source_ref: Option<&str>,
    existing_image_id: Option<&str>,
    _alt_text: &str,
    device_id: &str,
) -> Result<BookCoverMutation, AssetSourceError> {
    if source_ref.is_some() && existing_image_id.is_some() {
        return Err(AssetSourceError::new("conflicting_source"));
    }
    if source_ref.is_some() {
        return Err(AssetSourceError::new("source_requires_existing_image"));
    }
    let book = db::get_object(conn, book_id)
        .map_err(|_| AssetSourceError::new("storage"))?
        .ok_or_else(|| AssetSourceError::new("missing_book"))?;
    if book.type_id != BOOK {
        return Err(AssetSourceError::new("wrong_book_type"));
    }
    validate_object(&book)?;
    let mut image = None;
    let image_id = if let Some(id) = existing_image_id {
        let object = db::get_object(conn, id)
            .map_err(|_| AssetSourceError::new("storage"))?
            .ok_or_else(|| AssetSourceError::new("missing_image"))?;
        if object.type_id != IMAGE {
            return Err(AssetSourceError::new("wrong_image_type"));
        }
        if object.deleted_at.is_some() {
            return Err(AssetSourceError::new("deleted_image"));
        }
        validate_object(&object)?;
        image = Some(object);
        Some(id.to_string())
    } else {
        None
    };
    let existing: Vec<ObjectLink> = db::list_object_links(conn)
        .map_err(|_| AssetSourceError::new("storage"))?
        .into_iter()
        .filter(|l| l.source_object_id == book_id && l.link_type == COVER)
        .collect();
    let desired = image_id.as_ref().map(|id| canonical_link_id(book_id, id));
    let already = desired
        .as_ref()
        .and_then(|id| existing.iter().find(|l| &l.id == id))
        .is_some()
        && existing.len() == 1;
    if already {
        return Ok(BookCoverMutation {
            changed: false,
            book: Some(book),
            image,
            ..Default::default()
        });
    }
    conn.execute_batch("SAVEPOINT canonical_cover")
        .map_err(|_| AssetSourceError::new("storage"))?;
    let result = (|| {
        let mut mutation = BookCoverMutation {
            changed: true,
            book: Some(book.clone()),
            image: image.clone(),
            ..Default::default()
        };

        for link in existing {
            db::delete_object_link(conn, &link.id).map_err(|_| AssetSourceError::new("storage"))?;
            mutation.deleted_link_ids.push(link.id);
        }
        if let (Some(id), Some(link_id)) = (image_id, desired) {
            let link = ObjectLink {
                id: link_id,
                source_object_id: book_id.into(),
                target_object_id: id,
                link_type: COVER.into(),
                created_at: now(device_id),
            };
            db::upsert_object_link(conn, &link).map_err(|_| AssetSourceError::new("storage"))?;
            mutation.links.push(link);
        }
        Ok::<_, AssetSourceError>(mutation)
    })();
    match result {
        Ok(mutation) => {
            conn.execute_batch("RELEASE canonical_cover")
                .map_err(|_| AssetSourceError::new("storage"))?;
            Ok(mutation)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO canonical_cover; RELEASE canonical_cover");
            Err(error)
        }
    }
}

/// Canonical ingress for compatibility Todo/Project/Tag writes.  Adapters may
/// construct the legacy wire record, but persistence, HLC and link state stay
/// owned by this facade.
pub fn write_legacy_records(
    conn: &Connection,
    records: &[crate::canonical_types::compatibility::LegacyRecord],
    source_kind: &str,
    device_id: Option<String>,
) -> Result<Vec<SyncEntity>, String> {
    use crate::canonical_types::compatibility::{
        map_legacy_with_context, CanonicalIdentity, LegacySource, MappingContext,
    };
    use std::collections::BTreeMap;
    let mut existing_object_ids = BTreeMap::new();
    for object in db::list_objects(conn)? {
        existing_object_ids.insert(
            object.id,
            CanonicalIdentity::new(object.type_id, object.type_version),
        );
    }
    for record in records {
        let type_id = match record.legacy_type_id.as_str() {
            "task_obj" => "com.kosmos.task",
            "project_obj" => "com.kosmos.project",
            "tag_obj" => "com.kosmos.tag",
            _ => continue,
        };
        existing_object_ids.insert(record.id.clone(), CanonicalIdentity::new(type_id, VERSION));
    }
    let context = MappingContext {
        existing_object_ids,
        existing_links: db::list_object_links(conn)?,
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
        db::upsert_object(conn, &item.object)?;
        let hlc = db::bump_sync_version_vector(conn, "object", &id, write_device, false)?;
        conn.execute(
            concat!(
                "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON ",
                "CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE ",
                "excluded.hlc > object_sync_versions.hlc"
            ),
            params![id, hlc],
        )
        .map_err(|e| e.to_string())?;
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
            db::upsert_object_link(conn, &link)?;
            let hlc =
                db::bump_sync_version_vector(conn, "object_link", &link_id, write_device, false)?;
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
    let mut object = db::get_object(conn, id)?.ok_or_else(|| "MISSING_OBJECT".to_string())?;
    if object.type_id != expected_type_id {
        return Err("WRONG_TARGET".into());
    }
    let write_device = device_id.as_deref().unwrap_or("ark-core");
    object.deleted_at = Some(HLC::now(write_device).wall_time);
    db::upsert_object(conn, &object)?;
    let hlc = db::bump_sync_version_vector(conn, "object", id, write_device, true)?;
    conn.execute(
        concat!(
            "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,1) ON ",
            "CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=1 WHERE ",
            "excluded.hlc > object_sync_versions.hlc"
        ),
        params![id, hlc],
    )
    .map_err(|e| e.to_string())?;
    db::record_sync_tombstone_with_type(conn, "object", id, Some(&object.type_id), &hlc)?;
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
