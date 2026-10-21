use super::inventory::{compact, envelope, generic_inventory, hash_bytes, SourceEnvelope};
use super::inventory::{SourceKind, SourceRecord};
use crate::source_snapshot::SourceSnapshot;
use serde_json::{json, Value};

fn parse_json_field(raw: &str, row_id: &str, field: &str) -> Result<Value, String> {
    serde_json::from_str(raw)
        .map_err(|error| format!("invalid legacy todos.{field} for {}: {error}", row_id))
}

pub(crate) fn native_inventory(
    snapshot: &SourceSnapshot,
    out: &mut Vec<SourceRecord>,
) -> Result<(), String> {
    // Areas and headings are migrated as canonical project objects. Their
    // legacy kind, ordering and parent are carried in compatibility extensions
    // so the compatibility read view remains lossless after table retirement.
    for row in &snapshot.todos {
        let mut props = serde_json::Map::new();
        props.insert("notes".into(), json!(row.notes));
        props.insert("priority".into(), json!(row.priority));
        props.insert("scheduled_date".into(), json!(row.scheduled_date));
        props.insert("deadline".into(), json!(row.deadline));
        props.insert("reminder_date".into(), json!(row.reminder_date));
        if row.is_someday != 0 {
            props.insert("is_someday".into(), json!(true));
        }
        if row.is_completed != 0 {
            props.insert("is_completed".into(), json!(true));
        }
        props.insert("completed_at".into(), json!(row.completed_at));
        if row.is_cancelled != 0 {
            props.insert("is_cancelled".into(), json!(true));
        }
        props.insert("cancelled_at".into(), json!(row.cancelled_at));
        props.insert("heading_id".into(), json!(row.heading_id));
        if let Some(project) = &row.project_id {
            props.insert("project_id".into(), json!(project));
        }
        props.insert("area_id".into(), json!(row.area_id));
        props.insert(
            "tag_ids".into(),
            parse_json_field(&row.tag_ids, &row.id, "tag_ids")?,
        );
        props.insert(
            "checklist_items".into(),
            parse_json_field(&row.checklist_items, &row.id, "checklist_items")?,
        );
        props.insert(
            "recurrence_rule".into(),
            row.recurrence_rule
                .as_deref()
                .map(|value| parse_json_field(value, &row.id, "recurrence_rule"))
                .transpose()?
                .unwrap_or(Value::Null),
        );
        let value = envelope(SourceEnvelope {
            id: row.id.clone(),
            source_kind: "task_obj",
            title: row.title.clone(),
            content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
            props: Value::Object(props),
            created: row.created_at.clone(),
            updated: row.created_at.clone(),
            deleted: None,
        });
        push_native(out, "todos", row.id.clone(), compact(&value)?);
    }

    for row in &snapshot.projects {
        let value = envelope(SourceEnvelope {
            id: row.id.clone(),
            source_kind: "project_obj",
            title: row.title.clone(),
            content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
            props: json!({
                "notes": row.notes,
                "status": row.status,
                "scheduled_date": row.scheduled_date,
                "deadline": row.deadline,
                "color_tag": row.color_tag,
                "area_id": row.area_id
            }),
            created: row.created_at.clone(),
            updated: row.created_at.clone(),
            deleted: None,
        });
        push_native(out, "projects", row.id.clone(), compact(&value)?);
    }

    for row in &snapshot.tags {
        let value = envelope(SourceEnvelope {
            id: row.id.clone(),
            source_kind: "tag_obj",
            title: row.title.clone(),
            content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
            props: json!({"color": row.color}),
            created: row.created_at.clone(),
            updated: row.created_at.clone(),
            deleted: None,
        });
        push_native(out, "tags", row.id.clone(), compact(&value)?);
    }

    if let Some(areas) = &snapshot.areas {
        for row in areas {
            let bytes = compact(&envelope(SourceEnvelope {
                id: row.id.clone(),
                source_kind: "project_obj",
                title: row.title.clone(),
                content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
                props: json!({"legacy_kind":"area","sort_order":row.sort_order}),
                created: row.created_at.clone(),
                updated: row.created_at.clone(),
                deleted: None,
            }))?;
            push_native(out, "areas", row.id.clone(), bytes);
        }
    }

    if let Some(headings) = &snapshot.headings {
        for row in headings {
            let bytes = compact(&envelope(SourceEnvelope {
                id: row.id.clone(),
                source_kind: "project_obj",
                title: row.title.clone(),
                content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
                props: json!({
                    "legacy_kind":"heading",
                    "sort_order":row.sort_order,
                    "legacy_parent_project_id":row.project_id,
                    "related_ids":[row.project_id]
                }),
                created: "1970-01-01T00:00:00.000Z".into(),
                updated: "1970-01-01T00:00:00.000Z".into(),
                deleted: None,
            }))?;
            push_native(out, "headings", row.id.clone(), bytes);
        }
    }
    Ok(())
}

fn push_native(out: &mut Vec<SourceRecord>, kind: &str, id: String, bytes: Vec<u8>) {
    let hash = hash_bytes(&bytes);
    out.push(SourceRecord {
        source_kind: SourceKind::Native(kind.into()),
        source_id: id,
        source_hash: hash,
        canonical_bytes: bytes.clone(),
        raw_source: bytes,
    });
}

pub fn inventory_snapshot(snapshot: &SourceSnapshot) -> Result<Vec<SourceRecord>, String> {
    let mut out = Vec::new();
    let mut blocked = Vec::new();
    generic_inventory(snapshot, &mut out, &mut blocked)?;
    native_inventory(snapshot, &mut out)?;
    out.sort_by(|a, b| {
        (a.source_kind.name(), &a.source_id).cmp(&(b.source_kind.name(), &b.source_id))
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_snapshot::TodoRow;

    type InvalidField = (&'static str, fn(&mut TodoRow));

    fn todo() -> TodoRow {
        TodoRow {
            id: "todo-1".into(),
            title: "task".into(),
            notes: None,
            priority: 0,
            scheduled_date: None,
            deadline: None,
            reminder_date: None,
            is_today: 0,
            is_evening: 0,
            is_someday: 0,
            is_completed: 0,
            completed_at: None,
            is_cancelled: 0,
            cancelled_at: None,
            is_trashed: 0,
            sort_order: 0,
            heading_id: None,
            project_id: None,
            area_id: None,
            tag_ids: "[]".into(),
            checklist_items: "[]".into(),
            recurrence_rule: None,
            created_at: "2026-01-01T00:00:00.000Z".into(),
        }
    }

    #[test]
    fn malformed_native_task_json_blocks_projection_without_substitution() {
        let invalid_fields: [InvalidField; 3] = [
            ("tag_ids", |row| row.tag_ids = "not-json".into()),
            ("checklist_items", |row| {
                row.checklist_items = "not-json".into()
            }),
            ("recurrence_rule", |row| {
                row.recurrence_rule = Some("not-json".into())
            }),
        ];
        for (error_field, invalidate) in invalid_fields {
            let mut row = todo();
            invalidate(&mut row);
            let error = inventory_snapshot(&SourceSnapshot {
                todos: vec![row],
                ..SourceSnapshot::default()
            })
            .expect_err("invalid persisted JSON must block migration projection");
            assert!(error.contains("todo-1"));
            assert!(error.contains(error_field));
        }
    }
}
