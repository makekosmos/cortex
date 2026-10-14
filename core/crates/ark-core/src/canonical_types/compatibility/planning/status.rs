use super::super::shared::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// Legacy `task_obj` props (flat, from the Engine's todo API and sync) to
/// core.task 0.1.0 props. Calendar days come out bare, instants stay RFC 3339,
/// and the legacy recurrence weekday 0..=6 (Monday first) becomes ISO 1..=7.
pub(crate) fn task(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    let is_completed = flag(m, "is_completed", u)?;
    let is_cancelled = flag(m, "is_cancelled", u)?;
    let is_today = flag(m, "is_today", u)?;
    let is_someday = flag(m, "is_someday", u)?;
    let is_evening = flag(m, "is_evening", u)?;
    let status = match val(m, "status", &[], u)? {
        Some(v) => match v.as_str() {
            Some("triage" | "inbox") => "inbox",
            Some("todo" | "backlog") => "todo",
            Some("in_progress" | "inProgress") => "inProgress",
            Some("done") => "done",
            Some("canceled" | "duplicate") => "canceled",
            _ => return Err(invalid("/props/status")),
        },
        None if is_completed => "done",
        None if is_cancelled => "canceled",
        None => "inbox",
    };
    let state = match status {
        "done" => {
            if is_cancelled {
                return Err(conflict("/props/is_cancelled"));
            }
            let done_at = instant(m, "completedAt", &["completed_at"], u, at)?;
            json!({"kind": "done", "at": done_at})
        }
        "canceled" => {
            let canceled_at = instant(m, "canceledAt", &["cancelled_at"], u, at)?;
            json!({"kind": "canceled", "at": canceled_at})
        }
        "inProgress" => json!({"kind": "inProgress", "at": null}),
        _ => json!({"kind": "open", "at": null}),
    };
    // A completion stamp on a task that is not completed would be dropped.
    if status != "done" && !val_is_null(m, "completedAt", &["completed_at"], u)? {
        return Err(CompatFailure::DataLossRisk {
            pointer: "/props/completed_at".into(),
        });
    }
    let scheduled = day(m, "scheduledAt", &["scheduled_date"], u)?;
    let planning_kind = if is_today {
        "today"
    } else if is_someday {
        "someday"
    } else if scheduled.is_some() {
        "date"
    } else if status == "inbox" {
        "inbox"
    } else {
        "anytime"
    };
    let planning_date = (planning_kind == "date").then_some(scheduled).flatten();
    if planning_kind != "date" && scheduled_present(m, u)? {
        // Today or someday wins over a scheduled day; the day is not lost
        // because `extensions` below keeps the unconsumed legacy key.
        u.remove("scheduled_date");
        u.remove("scheduledAt");
    }
    o.insert("state".into(), state);
    o.insert(
        "planning".into(),
        json!({
            "kind": planning_kind,
            "period": if is_evening { "evening" } else { "any" },
            "date": planning_date,
        }),
    );
    o.insert("deadline".into(), day_value(m, "dueAt", &["deadline"], u)?);
    let reminder = instant_or_null(m, "reminderAt", &["reminder_date"], u)?;
    o.insert(
        "reminders".into(),
        match reminder {
            Value::Null => json!([]),
            at => json!([{"kind": "floating", "at": at, "timeZone": null}]),
        },
    );
    o.insert(
        "recurrence".into(),
        recurrence(val(m, "recurrence", &["recurrence_rule"], u)?)?,
    );
    o.insert("priority".into(), priority(m, u)?);
    o.insert(
        "checklist".into(),
        checklist(val(m, "checklist", &["checklist_items"], u)?)?,
    );
    put_extensions(o);
    relation_single_with_aliases(
        m,
        "projectId",
        &["project_id"],
        "project",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )?;
    relation_with_aliases(
        m,
        "tagIds",
        &["tag_ids"],
        "tag",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )?;
    relation_with_aliases(
        m,
        "relatedIds",
        &["related_ids"],
        "related",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )?;
    relation_with_aliases(
        m,
        "sourceNoteId",
        &["source_note_id"],
        "source-note",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )
}

fn flag(
    m: &Map<String, Value>,
    key: &str,
    u: &mut BTreeSet<String>,
) -> Result<bool, CompatFailure> {
    match val(m, key, &[&camel(key)], u)? {
        None | Some(Value::Null) => Ok(false),
        Some(v) => v.as_bool().ok_or_else(|| invalid(&format!("/props/{key}"))),
    }
}

fn val_is_null(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut BTreeSet<String>,
) -> Result<bool, CompatFailure> {
    Ok(matches!(val(m, c, a, u)?, None | Some(Value::Null)))
}

fn scheduled_present(
    m: &Map<String, Value>,
    u: &mut BTreeSet<String>,
) -> Result<bool, CompatFailure> {
    Ok(!val_is_null(m, "scheduledAt", &["scheduled_date"], u)?)
}

/// A required instant: the stored stamp, or the record's own update time.
fn instant(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut BTreeSet<String>,
    fallback: &str,
) -> Result<String, CompatFailure> {
    match instant_or_null(m, c, a, u)? {
        Value::Null => Ok(fallback.into()),
        v => Ok(v.as_str().unwrap_or(fallback).into()),
    }
}

fn instant_or_null(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut BTreeSet<String>,
) -> Result<Value, CompatFailure> {
    let raw = string(m, c, a, u, true)?;
    match raw.as_str() {
        None => Ok(Value::Null),
        Some(s) if chrono::DateTime::parse_from_rfc3339(s).is_ok() => Ok(raw),
        Some(_) => Err(invalid(&format!("/props/{c}"))),
    }
}

fn day(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut BTreeSet<String>,
) -> Result<Option<String>, CompatFailure> {
    Ok(day_value(m, c, a, u)?.as_str().map(str::to_owned))
}

/// A bare calendar day. A stamp keeps the day it expresses on the reader's clock,
/// the same rule the day-field normalizer uses; anything else is refused.
fn day_value(
    m: &Map<String, Value>,
    c: &str,
    a: &[&str],
    u: &mut BTreeSet<String>,
) -> Result<Value, CompatFailure> {
    let raw = string(m, c, a, u, true)?;
    if raw.is_null() {
        return Ok(Value::Null);
    }
    crate::canonical_types::normalize::day_value(&raw)
        .map(|day| json!(day))
        .ok_or_else(|| invalid(&format!("/props/{c}")))
}

fn priority(m: &Map<String, Value>, u: &mut BTreeSet<String>) -> Result<Value, CompatFailure> {
    Ok(match val(m, "priority", &[], u)? {
        None => json!("none"),
        Some(v) if v.is_number() => match v.as_i64() {
            Some(0) => json!("none"),
            Some(1) => json!("low"),
            Some(2) => json!("medium"),
            Some(3) => json!("high"),
            _ => return Err(invalid("/props/priority")),
        },
        Some(v)
            if matches!(
                v.as_str(),
                Some("none" | "low" | "medium" | "high" | "urgent")
            ) =>
        {
            v.clone()
        }
        Some(_) => return Err(invalid("/props/priority")),
    })
}

fn checklist(raw: Option<&Value>) -> Result<Value, CompatFailure> {
    let items: &[Value] = match raw {
        None | Some(Value::Null) => &[],
        Some(Value::Array(items)) => items,
        Some(_) => return Err(invalid("/props/checklist")),
    };
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("/props/checklist/id"))?;
        let text = item
            .get("text")
            .or_else(|| item.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let done = item
            .get("done")
            .or_else(|| item.get("isCompleted"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.push(json!({"id": id, "text": text, "done": done, "rank": format!("{index:06}")}));
    }
    Ok(Value::Array(out))
}

/// Legacy recurrence object to core.task recurrence (weekday 0 = Monday → 1).
fn recurrence(raw: Option<&Value>) -> Result<Value, CompatFailure> {
    let Some(rule) = raw.filter(|v| !v.is_null()) else {
        return Ok(Value::Null);
    };
    let object = rule
        .as_object()
        .ok_or_else(|| invalid("/props/recurrence"))?;
    let frequency = object
        .get("frequency")
        .and_then(Value::as_str)
        .filter(|f| ["daily", "weekly", "monthly", "yearly"].contains(f))
        .ok_or_else(|| invalid("/props/recurrence/frequency"))?;
    let kind = match object.get("recurrenceType").and_then(Value::as_str) {
        Some("afterCompletion") => "afterCompletion",
        Some("fixed") | None => "calendar",
        Some(_) => return Err(invalid("/props/recurrence/recurrenceType")),
    };
    let interval = object
        .get("interval")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let days = match object.get("daysOfWeek") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::Array(days)) => {
            let mut iso = Vec::with_capacity(days.len());
            for day in days {
                let d = day
                    .as_u64()
                    .filter(|d| *d <= 6)
                    .ok_or_else(|| invalid("/props/recurrence/daysOfWeek"))?;
                iso.push(json!(d + 1));
            }
            Value::Array(iso)
        }
        Some(_) => return Err(invalid("/props/recurrence/daysOfWeek")),
    };
    let end = match object.get("endDate").and_then(Value::as_str) {
        None => Value::Null,
        Some(stamp) => {
            let date = chrono::DateTime::parse_from_rfc3339(stamp)
                .map_err(|_| invalid("/props/recurrence/endDate"))?
                .date_naive()
                .to_string();
            json!({"kind": "until", "date": date, "count": null})
        }
    };
    Ok(json!({
        "kind": kind,
        "frequency": frequency,
        "interval": interval,
        "daysOfWeek": days,
        "timeZone": null,
        "end": end,
    }))
}
pub(crate) fn project(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    let s = string(m, "status", &[], u, true)?;
    if let Some(value) = s.as_str() {
        if !matches!(value, "active" | "someday" | "completed") {
            return Err(invalid("/props/status"));
        }
    }
    o.insert(
        "status".into(),
        if s.is_null() { json!("active") } else { s },
    );
    for (c, a) in [
        ("scheduledAt", "scheduled_date"),
        ("dueAt", "deadline"),
        ("color", "color"),
    ] {
        o.insert(c.into(), string(m, c, &[a], u, true)?);
    }
    put_extensions(o);
    relation_with_aliases(
        m,
        "tagIds",
        &["tag_ids"],
        "tag",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )?;
    relation_with_aliases(
        m,
        "relatedIds",
        &["related_ids"],
        "related",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )?;
    Ok(())
}
pub(crate) fn tag(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    o.insert("color".into(), string(m, "color", &[], u, true)?);
    put_extensions(o);
    relation_with_aliases(
        m,
        "relatedIds",
        &["related_ids", "related"],
        "related",
        LinkWrite {
            links: l,
            id,
            at,
            unknown: u,
        },
    )
}
