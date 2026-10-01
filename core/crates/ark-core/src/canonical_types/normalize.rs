//! Read/coercion boundary for stored canonical props.
//!
//! `format: "date"` fields are day-granularity. Under schema version 1.0.0 the
//! same fields were declared `date-time` but nothing enforced it, so stored
//! objects may carry RFC 3339 stamps. Normalizing a stamp keeps the calendar
//! day the stamp expresses — its leading `YYYY-MM-DD` — which is the day the
//! writer meant: there was never a real time-of-day behind these fields.
//!
//! This runs on read paths (delphi projection, export) and on the legacy
//! compat write mapping. The canonical write ingress stays strict: a bad
//! `date` value is rejected, never silently normalized into the store.

use serde_json::{Map, Value};

/// `Some(day)` for a stored day-field value: a bare `YYYY-MM-DD`, or the
/// calendar day an RFC 3339 stamp expresses. `None` for anything else, so
/// callers can still fail on genuinely malformed data.
pub fn day_value(value: &Value) -> Option<String> {
    let s = value.as_str()?;
    if crate::canonical_types::validation::is_full_date(s) {
        return Some(s.to_owned());
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.format("%Y-%m-%d").to_string())
}

/// Rewrite `props` in place: every schema property declared `format: "date"`
/// holding a parseable stamp becomes its expressed day. Recurses into nested
/// object properties (e.g. `recurrence.endDate`).
pub fn day_props(schema: &Value, props: &mut Map<String, Value>) {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, sub) in properties {
        if sub.get("format").and_then(Value::as_str) == Some("date") {
            if let Some(field) = props.get_mut(name) {
                if let Some(day) = day_value(field) {
                    *field = Value::String(day);
                }
            }
        }
        if let Some(child) = props.get_mut(name).and_then(Value::as_object_mut) {
            day_props(sub, child);
        }
    }
}
