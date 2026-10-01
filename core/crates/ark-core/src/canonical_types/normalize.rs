//! Read/coercion boundary for stored canonical props.
//!
//! `format: "date"` fields are day-granularity. Under schema version 1.0.0 the
//! same fields were declared `date-time` but nothing enforced it, so stored
//! objects may carry RFC 3339 stamps. Normalizing a stamp keeps the calendar
//! day the stamp expresses in the Engine's **local** zone: old writers emitted
//! `toISOString()` of a local midnight (e.g. `2026-05-15T21:00:00Z` is May 16
//! in Moscow), so the stored day only makes sense in the user's zone. The same
//! rule lives in agenda-gpui `model::date_only` — one interpretation per day
//! field, identical in both repos (the near-midnight test vectors are shared).
//!
//! This runs on read paths (delphi projection, export) and on the legacy
//! compat write mapping. The canonical write ingress stays strict: a bad
//! `date` value is rejected, never silently normalized into the store.

use serde_json::{Map, Value};

/// `Some(day)` for a stored day-field value: a bare `YYYY-MM-DD`, or the
/// calendar day an RFC 3339 stamp expresses in local time. `None` for anything
/// else, so callers can still fail on genuinely malformed data.
pub fn day_value(value: &Value) -> Option<String> {
    let s = value.as_str()?;
    if crate::canonical_types::validation::is_full_date(s) {
        return Some(s.to_owned());
    }
    stamp_day_at(s, *chrono::Local::now().offset())
}

/// The day `stamp` expresses at `offset` — pure, so tests pin the zone instead
/// of relying on the machine's local time.
fn stamp_day_at(stamp: &str, offset: chrono::FixedOffset) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(stamp)
        .ok()
        .map(|d| d.with_timezone(&offset).format("%Y-%m-%d").to_string())
}

/// Rewrite `props` in place: every schema property declared `format: "date"`
/// holding a parseable stamp becomes the day it expresses locally. Recurses
/// into nested object properties (e.g. `recurrence.endDate`).
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

#[cfg(test)]
mod tests {
    use super::*;

    // Shared with agenda-gpui `model::tests`: the day a stamp expresses is the
    // day in the reader's local zone — e.g. `21:00Z` is already the next day
    // at UTC+03. Both repos pin `FixedOffset` to keep the vectors deterministic.
    #[test]
    fn stamp_day_uses_the_reader_zone_not_the_stamp_zone() {
        let cases: &[(&str, i32, &str)] = &[
            ("2026-05-15T21:00:00Z", 3 * 3600, "2026-05-16"),
            ("2026-05-15T21:00:00Z", -5 * 3600, "2026-05-15"),
            ("2026-05-15T23:30:00+03:00", 3 * 3600, "2026-05-15"),
            ("2026-05-15T23:30:00+03:00", -5 * 3600, "2026-05-15"),
            ("2026-05-16T00:30:00-05:00", -5 * 3600, "2026-05-16"),
            ("2026-05-16T00:30:00-05:00", 14 * 3600, "2026-05-16"),
        ];
        for (stamp, secs, expected) in cases {
            let offset = chrono::FixedOffset::east_opt(*secs).unwrap();
            assert_eq!(
                stamp_day_at(stamp, offset).as_deref(),
                Some(*expected),
                "{stamp} at offset {secs}"
            );
        }
        for bad in ["soon", "2026-13-40", "2026-05-15T25:00:00Z"] {
            assert!(
                stamp_day_at(bad, chrono::FixedOffset::east_opt(0).unwrap()).is_none(),
                "{bad}"
            );
        }
    }

    #[test]
    fn day_value_normalizes_dates_and_stamps() {
        use serde_json::json;
        assert_eq!(
            day_value(&json!("2026-05-15")).as_deref(),
            Some("2026-05-15")
        );
        assert!(day_value(&json!("soon")).is_none());
        assert!(day_value(&json!(null)).is_none());
    }
}
