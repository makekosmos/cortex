use super::shared::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
pub fn time_entry(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    q: &mut Map<String, Value>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    o.insert(
        "startedAt".into(),
        string(m, "startedAt", &["started_at"], u, false)?,
    );
    o.insert(
        "endedAt".into(),
        string(m, "endedAt", &["ended_at"], u, true)?,
    );
    o.insert("billable".into(), boolv(m, "billable", &[], u)?);
    let s = val(m, "source", &[], u)?;
    o.insert(
        "source".into(),
        match s {
            None => json!("manual"),
            Some(v) if matches!(v.as_str(), Some("manual" | "pomodoro" | "imported")) => v.clone(),
            Some(_) => return Err(invalid("/props/source")),
        },
    );
    o.insert(
        "taskTitle".into(),
        string(m, "taskTitle", &["task_title"], u, true)?,
    );
    put_extensions(o);
    relation_single_with_aliases(m, "taskId", &["task_id"], "for-task", l, id, at, u)?;
    relation_with_aliases(m, "tagIds", &["tag_ids"], "tag", l, id, at, u)?;
    for k in [
        "provider",
        "accountId",
        "account_id",
        "externalId",
        "external_id",
        "projectId",
        "project_id",
        "workspaceId",
        "workspace_id",
    ] {
        if let Some(v) = val(m, k, &[], u)? {
            q.insert(camel(k), v.clone());
        }
    }
    Ok(())
}
pub fn game(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    local: &mut Map<String, Value>,
    q: &mut Map<String, Value>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    let s = val(m, "playStatus", &["play_status"], u)?;
    o.insert(
        "playStatus".into(),
        match s {
            None | Some(Value::Null) => Value::Null,
            Some(v) => match v.as_str() {
                Some("not_started") => json!("notStarted"),
                Some("in_progress") => json!("inProgress"),
                Some("completed" | "abandoned") => v.clone(),
                _ => return Err(invalid("/props/playStatus")),
            },
        },
    );
    for (c, a) in [
        ("userRating", "user_rating"),
        ("released", "released"),
        ("description", "description"),
    ] {
        if let Some(v) = val(m, c, &[a], u)? {
            if c == "userRating"
                && (!v.is_number() || v.as_f64().is_some_and(|n| !(0.0..=10.0).contains(&n)))
                && !v.is_null()
            {
                return Err(invalid("/props/userRating"));
            }
            if c != "userRating" && !v.is_string() && !v.is_null() {
                return Err(invalid(&format!("/props/{c}")));
            }
            o.insert(c.into(), v.clone());
        } else {
            o.insert(c.into(), Value::Null);
        }
    }
    o.insert(
        "genres".into(),
        array_or_null(m, "genres", &[], u, json!([]))?,
    );
    o.insert(
        "platforms".into(),
        array_or_null(m, "platforms", &[], u, json!([]))?,
    );
    put_extensions(o);
    for (c, a) in [
        ("exePath", "exe_path"),
        ("installDir", "install_dir"),
        ("savePath", "save_path"),
        ("saveExists", "save_exists"),
        ("exeName", "exe_name"),
    ] {
        if let Some(v) = val(m, c, &[a], u)? {
            local
                .entry("game")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .map(|nested| nested.insert(c.into(), v.clone()));
        }
    }
    for (c, a) in [
        ("totalPlaytimeSeconds", "total_playtime_seconds"),
        ("lastPlayedAt", "last_played_at"),
        ("playCount", "play_count"),
    ] {
        if let Some(v) = val(m, c, &[a], u)? {
            let valid = match c {
                "lastPlayedAt" => v.is_string() || v.is_null(),
                _ => v.as_i64().is_some_and(|n| n >= 0),
            };
            if !valid {
                return Err(invalid(&format!("/props/{c}")));
            }
            local
                .entry("game")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .map(|nested| {
                    nested
                        .entry("legacyAggregates")
                        .or_insert_with(|| json!({}))
                        .as_object_mut()
                        .map(|aggregate| aggregate.insert(c.into(), v.clone()));
                });
        }
    }
    for (c, a) in [
        ("source", "source_app"),
        ("rawgId", "rawg_id"),
        ("coverImage", "cover_image"),
        ("backgroundImage", "background_image"),
    ] {
        if let Some(v) = val(m, c, &[a], u)? {
            q.insert(c.into(), v.clone());
        }
    }
    relation_with_aliases(m, "noteIds", &["note_ids"], "note", l, id, at, u)?;
    relation_with_aliases(m, "taskIds", &["task_ids"], "task", l, id, at, u)?;
    relation_with_aliases(m, "tagIds", &["tag_ids"], "tag", l, id, at, u)
}
