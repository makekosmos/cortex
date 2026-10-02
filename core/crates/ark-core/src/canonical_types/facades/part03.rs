
pub fn apply_legacy_compat_entity(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    let current: Option<String> = conn
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id=?1",
            params![entity.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if current
        .as_deref()
        .is_some_and(|hlc| HLC::compare_str(&entity.hlc, hlc) != std::cmp::Ordering::Greater)
    {
        return Ok(());
    }
    let legacy_type = match entity.entity_type.as_str() {
        "todo" => "task_obj",
        "project" => "project_obj",
        "tag" => "tag_obj",
        "area" | "heading" => return Err("LegacyPlanningReadOnly".into()),
        _ => return Err(format!("unknown sync entity type '{}'", entity.entity_type)),
    };
    let mut props = entity.data.clone();
    if let Some(title) = props.get("title") {
        if !title.is_string() {
            return Err("invalid type at /title".into());
        }
    }
    let title = props
        .remove("title")
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    props.retain(|key, value| {
        !value.is_null()
            && !(matches!(
                key.as_str(),
                "is_completed"
                    | "isCompleted"
                    | "is_cancelled"
                    | "isCancelled"
                    | "is_someday"
                    | "isSomeday"
            ) && value.as_bool() == Some(false))
    });
    let created_at = props
        .get("createdAt")
        .or_else(|| props.get("created_at"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let updated_at = props
        .get("updatedAt")
        .or_else(|| props.get("updated_at"))
        .and_then(Value::as_str)
        .unwrap_or(&entity.hlc)
        .to_owned();
    // Historical planning rows did not carry rich-text content.  The
    // compatibility contract supplies the canonical empty document rather
    // than rejecting an otherwise valid legacy wire payload.
    let source = json!({"id": entity.id, "legacy_type_id": legacy_type, "title": title, "content": {"type":"doc","content":[{"type":"paragraph"}]}, "props": props, "created_at": created_at, "updated_at": updated_at, "deleted_at": null});
    let mapped = crate::canonical_types::compatibility::map_legacy_source(
        crate::canonical_types::compatibility::LegacySource {
            source_kind: &entity.entity_type,
            source_id: &entity.id,
            raw_json: source.to_string().as_bytes(),
        },
    )
    .map_err(|e| format!("invalid type at {}", e.pointer()))?;
    db::upsert_object(conn, &mapped.object)?;
    for link in mapped.links {
        db::upsert_object_link(conn, &link)?;
    }
    conn.execute(concat!("INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON ","CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE ","excluded.hlc > object_sync_versions.hlc"), params![entity.id, entity.hlc]).map_err(|e| e.to_string())?;
    db::delete_sync_tombstone(conn, &entity.id)
}

pub fn apply_canonical_object_entity(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    let current = db::get_object_revision(conn, &entity.id)?;
    if current
        .as_deref()
        .is_some_and(|hlc| HLC::compare_str(&entity.hlc, hlc) != std::cmp::Ordering::Greater)
    {
        return Ok(());
    }
    conn.execute_batch("SAVEPOINT canonical_sync_object")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        if entity.deleted == Some(true) {
            db::delete_object(conn, &entity.id)?;
            conn.execute(
                    concat!("INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,1) ON ","CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=1 WHERE ","excluded.hlc > object_sync_versions.hlc"),
                    params![entity.id, entity.hlc],
                ).map_err(|e| e.to_string())?;
            db::upsert_sync_tombstone(conn, entity)?;
            return Ok(());
        }
        let mut data = entity.data.clone();
        data.insert("id".into(), Value::String(entity.id.clone()));
        let input: ArkObjectWrite =
            serde_json::from_value(Value::Object(data.clone())).map_err(|e| e.to_string())?;
        let input_type_id = input.type_id.clone();
        let input_type_version = input.type_version.clone();
        let canonical_alias = match input_type_id.as_str() {
            "note_obj" => Some("com.kosmos.note"),
            "task_obj" => Some("com.kosmos.task"),
            "project_obj" => Some("com.kosmos.project"),
            "tag_obj" => Some("com.kosmos.tag"),
            "person_obj" => Some("com.kosmos.person"),
            "image_obj" => Some("com.kosmos.image"),
            "time_entry_obj" => Some("com.kosmos.time-entry"),
            "game_obj" => Some("com.kosmos.game"),
            "book_obj" => Some("com.kosmos.book"),
            _ => None,
        };
        let object = if let Some(canonical_type_id) = canonical_alias {
            let props = input.props_json.clone();
            let source = serde_json::json!({
                "id": input.id,
                "legacy_type_id": input.type_id,
                "title": input.title,
                "content": input.content_json,
                "props": props,
                "created_at": input.created_at,
                "updated_at": input.updated_at,
                "deleted_at": input.deleted_at,
            });
            let mapped = crate::canonical_types::compatibility::map_legacy_source(
                crate::canonical_types::compatibility::LegacySource {
                    source_kind: "object",
                    source_id: &entity.id,
                    raw_json: source.to_string().as_bytes(),
                },
            )
            .map_err(|e| format!("{} at {}", e.code(), e.pointer()))?;
            debug_assert_eq!(mapped.object.type_id, canonical_type_id);
            mapped.object
        } else if crate::canonical_types::definitions::canonical_type_registrations()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|registration| registration.type_id == input_type_id)
        {
            crate::canonical_types::ingress::prepare_object(conn, input)
                .map_err(|e| e.to_string())?
        } else {
            // Generic Phase 2 objects have no canonical content contract.
            // Preserve the wire version exactly; resolving a legacy wire
            // version to a synthetic compatibility hash here changes the
            // pending/version contract.
            input.with_type_version(
                input_type_version
                    .unwrap_or_else(|| crate::type_registry::LEGACY_VERSION.to_string()),
            )
        };
        db::upsert_object(conn, &object)?;
        if let Some(raw_links) = data.get("links") {
            let links: Vec<ObjectLink> =
                serde_json::from_value(raw_links.clone()).map_err(|e| e.to_string())?;
            for link in links {
                db::upsert_object_link(conn, &link)?;
            }
        }
        conn.execute(
                concat!("INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON ","CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE ","excluded.hlc > object_sync_versions.hlc"),
                params![object.id, entity.hlc],
            ).map_err(|e| e.to_string())?;
        db::delete_sync_tombstone(conn, &entity.id)
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE canonical_sync_object")
            .map_err(|e| e.to_string()),
        Err(error) => {
            let _ = conn
                .execute_batch("ROLLBACK TO canonical_sync_object; RELEASE canonical_sync_object");
            Err(error)
        }
    }
}
