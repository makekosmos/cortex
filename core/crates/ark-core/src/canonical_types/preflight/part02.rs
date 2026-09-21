
fn native_inventory(conn: &Connection, out: &mut Vec<SourceRecord>) -> Result<(), String> {
    // Areas and headings are migrated as canonical project objects. Their
    // legacy kind, ordering and parent are carried in compatibility extensions
    // so the compatibility read view remains lossless after table retirement.
    let mut todo = conn.prepare("SELECT id,title,notes,priority,scheduled_date,deadline,reminder_date,is_someday,is_completed,completed_at,is_cancelled,cancelled_at,heading_id,project_id,area_id,tag_ids,checklist_items,recurrence_rule,created_at FROM todos ORDER BY id").map_err(|e| e.to_string())?;
    for row in todo
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, String>(15)?,
                r.get::<_, String>(16)?,
                r.get::<_, Option<String>>(17)?,
                r.get::<_, String>(18)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (
            id,
            title,
            notes,
            priority,
            scheduled,
            deadline,
            reminder,
            someday,
            completed,
            completed_at,
            cancelled,
            cancelled_at,
            heading,
            project,
            area,
            tags,
            checklist,
            recurrence,
            created,
        ) = row.map_err(|e| e.to_string())?;
        let mut props = serde_json::Map::new();
        props.insert("notes".into(), json!(notes));
        props.insert("priority".into(), json!(priority));
        props.insert("scheduled_date".into(), json!(scheduled));
        props.insert("deadline".into(), json!(deadline));
        props.insert("reminder_date".into(), json!(reminder));
        if someday != 0 {
            props.insert("is_someday".into(), json!(true));
        }
        if completed != 0 {
            props.insert("is_completed".into(), json!(true));
        }
        props.insert("completed_at".into(), json!(completed_at));
        if cancelled != 0 {
            props.insert("is_cancelled".into(), json!(true));
        }
        props.insert("cancelled_at".into(), json!(cancelled_at));
        props.insert("heading_id".into(), json!(heading));
        if let Some(project) = project {
            props.insert("project_id".into(), json!(project));
        }
        props.insert("area_id".into(), json!(area));
        props.insert(
            "tag_ids".into(),
            serde_json::from_str::<Value>(&tags).unwrap_or(Value::Array(Vec::new())),
        );
        props.insert(
            "checklist_items".into(),
            serde_json::from_str::<Value>(&checklist).unwrap_or(Value::Array(Vec::new())),
        );
        props.insert(
            "recurrence_rule".into(),
            recurrence
                .and_then(|v| serde_json::from_str::<Value>(&v).ok())
                .unwrap_or(Value::Null),
        );
        let props = Value::Object(props);
        let value = envelope(
            id.clone(),
            "task_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            props,
            created.clone(),
            created,
            None,
        );
        let bytes = compact(&value)?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("todos".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let mut projects=conn.prepare("SELECT id,title,notes,status,scheduled_date,deadline,color_tag,area_id,created_at FROM projects ORDER BY id").map_err(|e|e.to_string())?;
    for row in projects
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, notes, status, scheduled, deadline, color, area, created) =
            row.map_err(|e| e.to_string())?;
        let value = envelope(
            id.clone(),
            "project_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({"notes":notes,"status":status,"scheduled_date":scheduled,"deadline":deadline,"color_tag":color,"area_id":area}),
            created.clone(),
            created,
            None,
        );
        let bytes = compact(&value)?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("projects".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let mut tags = conn
        .prepare("SELECT id,title,color,created_at FROM tags ORDER BY id")
        .map_err(|e| e.to_string())?;
    for row in tags
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, color, created) = row.map_err(|e| e.to_string())?;
        let value = envelope(
            id.clone(),
            "tag_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({"color":color}),
            created.clone(),
            created,
            None,
        );
        let bytes = compact(&value)?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("tags".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    let has_table = |name: &str| {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
    };
    if has_table("areas") {
      let mut areas = conn
        .prepare("SELECT id,title,sort_order,created_at FROM areas ORDER BY id")
        .map_err(|e| e.to_string())?;
    for row in areas
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, sort_order, created) = row.map_err(|e| e.to_string())?;
        let bytes = compact(&envelope(
            id.clone(),
            "project_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({"legacy_kind":"area","sort_order":sort_order}),
            created.clone(),
            created,
            None,
        ))?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("areas".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    }
    if has_table("headings") {
    let mut headings = conn
        .prepare("SELECT id,title,sort_order,project_id FROM headings ORDER BY id")
        .map_err(|e| e.to_string())?;
    for row in headings
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, title, sort_order, project) = row.map_err(|e| e.to_string())?;
        let bytes = compact(&envelope(
            id.clone(),
            "project_obj",
            title,
            json!({"type":"doc","content":[{"type":"paragraph"}]}),
            json!({
                "legacy_kind":"heading",
                "sort_order":sort_order,
                "legacy_parent_project_id":project,
                "related_ids":[project]
            }),
            "1970-01-01T00:00:00.000Z".into(),
            "1970-01-01T00:00:00.000Z".into(),
            None,
        ))?;
        out.push(SourceRecord {
            source_kind: SourceKind::Native("headings".into()),
            source_id: id,
            source_hash: hash_bytes(&bytes),
            canonical_bytes: bytes.clone(),
            raw_source: bytes,
        });
    }
    }
    Ok(())
}

pub fn inventory_sources(conn: &Connection) -> Result<Vec<SourceRecord>, String> {
    let mut out = Vec::new();
    let mut blocked = Vec::new();
    generic_inventory(conn, &mut out, &mut blocked)?;
    native_inventory(conn, &mut out)?;
    out.sort_by(|a, b| {
        (a.source_kind.name(), &a.source_id).cmp(&(b.source_kind.name(), &b.source_id))
    });
    Ok(out)
}
