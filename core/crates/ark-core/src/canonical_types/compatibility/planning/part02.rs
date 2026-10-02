fn status(
    m: &Map<String, Value>,
    u: &mut BTreeSet<String>,
) -> Result<(String, Option<String>), CompatFailure> {
    let raw = val(m, "status", &[], u)?;
    if let Some(v) = raw {
        let s = v.as_str().ok_or_else(|| invalid("/props/status"))?;
        let out = match s {
            "triage" => "inbox",
            "todo" => "todo",
            "in_progress" => "inProgress",
            "done" => "done",
            "canceled" | "duplicate" => "canceled",
            "backlog" => "todo",
            "inbox" | "inProgress" => s,
            _ => return Err(invalid("/props/status")),
        };
        return Ok((out.into(), (s == "backlog").then_some("backlog".into())));
    }
    for (k, out) in [
        ("is_completed", "done"),
        ("is_cancelled", "canceled"),
        ("is_someday", "todo"),
    ] {
        if let Some(v) = val(m, k, &[&camel(k)], u)? {
            if v.as_bool() != Some(true) {
                return Err(invalid(&format!("/props/{k}")));
            }
            return Ok((out.into(), (k == "is_someday").then_some("backlog".into())));
        }
    }
    Ok(("inbox".into(), None))
}
pub(crate) fn task(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    let (s, b) = status(m, u)?;
    o.insert("status".into(), json!(s));
    if let Some(bucket) = b {
        o.insert("extensions".into(), json!({"kosmos":{"taskBucket":bucket}}));
    }
    let p = match val(m, "priority", &[], u)? {
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
    };
    o.insert("priority".into(), p);
    for (c, a) in [
        ("scheduledAt", "scheduled_date"),
        ("dueAt", "deadline"),
        ("reminderAt", "reminder_date"),
        ("completedAt", "completed_at"),
        ("canceledAt", "cancelled_at"),
    ] {
        o.insert(c.into(), string(m, c, &[a], u, true)?);
    }
    o.insert(
        "recurrence".into(),
        val(m, "recurrence", &["recurrence_rule"], u)?
            .cloned()
            .unwrap_or(Value::Null),
    );
    o.insert(
        "checklist".into(),
        array_or_null(m, "checklist", &["checklist_items"], u, json!([]))?,
    );
    put_extensions(o);
    relation_single_with_aliases(m, "projectId", &["project_id"], "project", LinkWrite { links: l, id, at, unknown: u })?;
    relation_with_aliases(m, "tagIds", &["tag_ids"], "tag", LinkWrite { links: l, id, at, unknown: u })?;
    relation_with_aliases(m, "relatedIds", &["related_ids"], "related", LinkWrite { links: l, id, at, unknown: u })?;
    relation_with_aliases(m, "sourceNoteId", &["source_note_id"], "source-note", LinkWrite { links: l, id, at, unknown: u })
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
    relation_with_aliases(m, "tagIds", &["tag_ids"], "tag", LinkWrite { links: l, id, at, unknown: u })?;
    relation_with_aliases(m, "relatedIds", &["related_ids"], "related", LinkWrite { links: l, id, at, unknown: u })?;
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
    relation_with_aliases(m, "relatedIds", &["related_ids", "related"], "related", LinkWrite { links: l, id, at, unknown: u })
}
