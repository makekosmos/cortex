use super::shared::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

use crate::canonical_types::definitions::canonical_type_registrations;
use crate::canonical_types::validation::validate_canonical;
use crate::delphi::types::TodoItem;
use crate::types::{ArkObject, ObjectLink};

/// Project one validated canonical task and its links into Delphi's read view.
///
/// This is deliberately a pure boundary: it does not deserialize the Delphi
/// shape, consult persistence, or infer project membership from task props.
pub fn project_task_to_delphi(
    object: &ArkObject,
    links: &[ObjectLink],
) -> Result<TodoItem, CompatibilityError> {
    let registration = canonical_type_registrations()
        .map_err(|_| projection_error(object, "", "CONFLICTING_FIELDS"))?
        .into_iter()
        .find(|registration| {
            registration.type_id == object.type_id && registration.version == object.type_version
        })
        .ok_or_else(|| projection_error(object, "/typeVersion", "INVALID_FIELD"))?;
    validate_canonical(&registration, &object.props_json, &object.content_json)
        .map_err(|error| projection_error(object, &error.pointer, "INVALID_FIELD"))?;
    let props = object
        .props_json
        .as_object()
        .ok_or_else(|| projection_error(object, "/props", "INVALID_FIELD"))?;
    let extensions = props
        .get("extensions")
        .and_then(Value::as_object)
        .ok_or_else(|| projection_error(object, "/props/extensions", "INVALID_FIELD"))?;
    let empty = Map::new();
    let compatibility = match extensions.get("compatibility") {
        None => &empty,
        Some(value) => value.as_object().ok_or_else(|| {
            projection_error(object, "/props/extensions/compatibility", "INVALID_FIELD")
        })?,
    };
    let planning = match compatibility.get("planning") {
        None => &empty,
        Some(value) => value.as_object().ok_or_else(|| {
            projection_error(
                object,
                "/props/extensions/compatibility/planning",
                "INVALID_FIELD",
            )
        })?,
    };
    let is_today = extension_bool(object, planning, "isToday")?;
    let _is_evening = extension_bool(object, planning, "isEvening")?;
    let sort_order = extension_i64(object, planning, "sortOrder")?.unwrap_or(0);
    let task_bucket = extensions
        .get("kosmos")
        .and_then(Value::as_object)
        .and_then(|mundus| mundus.get("taskBucket"));
    if let Some(value) = task_bucket {
        if !value.is_string() {
            return Err(projection_error(
                object,
                "/props/extensions/kosmos/taskBucket",
                "INVALID_FIELD",
            ));
        }
    }
    let mut projects: Vec<&ObjectLink> = links
        .iter()
        .filter(|link| link.source_object_id == object.id && link.link_type == "project")
        .collect();
    projects.sort_by(|a, b| (&a.target_object_id, &a.id).cmp(&(&b.target_object_id, &b.id)));
    if projects.len() > 1 {
        return Err(projection_error(
            object,
            "/links/project",
            "CONFLICTING_FIELDS",
        ));
    }
    let status = props
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| projection_error(object, "/props/status", "INVALID_FIELD"))?;
    Ok(TodoItem {
        id: object.id.clone(),
        title: object.title.clone(),
        // `scheduledAt` is a day field; objects written under schema 1.0.0 may
        // still carry the RFC 3339 stamp the old contract declared.
        scheduled_date: day_or_none(object, props, "scheduledAt")?,
        is_today,
        is_someday: task_bucket.and_then(Value::as_str) == Some("backlog"),
        is_completed: status == "done",
        completed_at: string_or_none(object, props, "completedAt")?,
        is_cancelled: status == "canceled",
        cancelled_at: string_or_none(object, props, "canceledAt")?,
        is_trashed: object.deleted_at.is_some(),
        sort_order,
        created_at: object.created_at.clone(),
        project_id: projects.first().map(|link| link.target_object_id.clone()),
    })
}

fn projection_error(object: &ArkObject, pointer: &str, code: &str) -> CompatibilityError {
    let raw_source = Vec::new();
    match code {
        "CONFLICTING_FIELDS" => CompatibilityError::ConflictingFields {
            source_kind: object.type_id.clone(),
            source_id: object.id.clone(),
            pointer: pointer.into(),
            raw_source,
        },
        _ => CompatibilityError::InvalidField {
            source_kind: object.type_id.clone(),
            source_id: object.id.clone(),
            pointer: pointer.into(),
            raw_source,
        },
    }
}

fn extension_bool(
    object: &ArkObject,
    parent: &Map<String, Value>,
    key: &str,
) -> Result<bool, CompatibilityError> {
    parent
        .get(key)
        .and_then(Value::as_bool)
        .or_else(|| {
            if parent.contains_key(key) {
                None
            } else {
                Some(false)
            }
        })
        .ok_or_else(|| {
            projection_error(
                object,
                &format!("/props/extensions/compatibility/planning/{key}"),
                "INVALID_FIELD",
            )
        })
}

fn extension_i64(
    object: &ArkObject,
    parent: &Map<String, Value>,
    key: &str,
) -> Result<Option<i64>, CompatibilityError> {
    match parent.get(key) {
        None => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or_else(|| {
            projection_error(
                object,
                &format!("/props/extensions/compatibility/planning/{key}"),
                "INVALID_FIELD",
            )
        }),
    }
}

fn day_or_none(
    object: &ArkObject,
    props: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, CompatibilityError> {
    match props.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => crate::canonical_types::normalize::day_value(value)
            .map(Some)
            .ok_or_else(|| projection_error(object, &format!("/props/{key}"), "INVALID_FIELD")),
    }
}

fn string_or_none(
    object: &ArkObject,
    props: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, CompatibilityError> {
    match props.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .map(Some)
            .ok_or_else(|| projection_error(object, &format!("/props/{key}"), "INVALID_FIELD")),
    }
}
