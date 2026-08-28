use super::*;

fn ffi_legacy_record(
    id: &str,
    legacy_type_id: &str,
    title: &str,
    props: Value,
    created_at: &str,
) -> crate::canonical_types::compatibility::LegacyRecord {
    crate::canonical_types::compatibility::LegacyRecord {
        id: id.into(),
        legacy_type_id: legacy_type_id.into(),
        title: title.into(),
        content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props,
        created_at: created_at.into(),
        updated_at: created_at.into(),
        deleted_at: None,
    }
}

fn ffi_clean_planning_props(mut props: Value) -> Value {
    if let Some(map) = props.as_object_mut() {
        map.retain(|key, value| {
            !value.is_null()
                && !(matches!(
                    key.as_str(),
                    "is_completed" | "is_cancelled" | "is_someday" | "is_today" | "is_evening"
                ) && value.as_bool() == Some(false))
        });
    }
    props
}

pub(super) fn ffi_todo_record(
    todo: &TodoItem,
) -> crate::canonical_types::compatibility::LegacyRecord {
    ffi_legacy_record(
        &todo.id,
        "task_obj",
        &todo.title,
        ffi_clean_planning_props(json!({
            "priority": todo.priority, "scheduled_date": todo.scheduled_date,
            "deadline": todo.deadline, "reminder_date": todo.reminder_date,
            "is_today": todo.is_today, "is_evening": todo.is_evening,
            "is_someday": todo.is_someday, "is_completed": todo.is_completed,
            "completed_at": todo.completed_at, "is_cancelled": todo.is_cancelled,
            "cancelled_at": todo.cancelled_at, "checklist_items": todo.checklist_items,
            "recurrence_rule": todo.recurrence_rule, "project_id": todo.project_id,
            "tag_ids": todo.tag_ids,
        })),
        &todo.created_at,
    )
}

pub(super) fn ffi_project_record(
    project: &Project,
) -> crate::canonical_types::compatibility::LegacyRecord {
    ffi_legacy_record(
        &project.id,
        "project_obj",
        &project.title,
        json!({
            "status": project.status, "scheduled_date": project.scheduled_date,
            "deadline": project.deadline, "color": project.color_tag,
        }),
        &project.created_at,
    )
}

pub(super) fn ffi_tag_record(tag: &Tag) -> crate::canonical_types::compatibility::LegacyRecord {
    ffi_legacy_record(
        &tag.id,
        "tag_obj",
        &tag.title,
        json!({"color": tag.color}),
        &tag.created_at,
    )
}

pub(super) fn ffi_write_legacy_records(
    conn: &Connection,
    records: Vec<crate::canonical_types::compatibility::LegacyRecord>,
) -> std::result::Result<Vec<SyncEntity>, String> {
    conn.execute_batch("SAVEPOINT ffi_legacy_write")
        .map_err(|e| e.to_string())?;
    let result = crate::canonical_types::facades::write_legacy_records(
        conn,
        &records,
        "ffi",
        Some("ark-core-ffi".into()),
    );
    match result {
        Ok(entities) => {
            conn.execute_batch("RELEASE ffi_legacy_write")
                .map_err(|e| e.to_string())?;
            Ok(entities)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO ffi_legacy_write; RELEASE ffi_legacy_write");
            Err(error)
        }
    }
}

pub(super) fn ffi_delete_planning_object(
    conn: &Connection,
    id: &str,
    expected_type_id: &str,
) -> std::result::Result<(), String> {
    conn.execute_batch("SAVEPOINT ffi_legacy_delete")
        .map_err(|e| e.to_string())?;
    let result = crate::canonical_types::facades::delete_legacy_object(
        conn,
        id,
        expected_type_id,
        Some("ark-core-ffi".into()),
    )
    .map(|_| ());
    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE ffi_legacy_delete")
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO ffi_legacy_delete; RELEASE ffi_legacy_delete");
            Err(error)
        }
    }
}

// Rust-only helpers (not exposed to UniFFI).
