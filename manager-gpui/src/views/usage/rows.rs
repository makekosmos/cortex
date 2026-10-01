//! Usage view data layer: row filtering (system processes), column sorting
//! and the duration/date formatting the table cells render.

use gpui::SharedString;
use serde_json::Value;

use crate::app::ManagerApp;
use crate::widgets::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UsageColumn {
    Name,
    Active,
    Sessions,
    LastSeen,
    Path,
}

#[derive(Clone, Copy, Debug)]
pub struct UsageSort {
    pub column: UsageColumn,
    pub ascending: bool,
}

impl Default for UsageSort {
    fn default() -> Self {
        Self {
            column: UsageColumn::Active,
            ascending: false,
        }
    }
}

impl UsageSort {
    /// Click on a header: same column flips direction, a new column takes its
    /// natural direction (text asc, numbers/time desc).
    fn toggled(&mut self, column: UsageColumn) {
        if self.column == column {
            self.ascending = !self.ascending;
        } else {
            self.column = column;
            self.ascending = matches!(column, UsageColumn::Name | UsageColumn::Path);
        }
    }
}

impl ManagerApp {
    pub fn toggle_usage_sort(&mut self, column: UsageColumn) {
        self.usage_sort.toggled(column);
        self.rebuild_usage_rows();
    }

    pub fn set_usage_show_system(&mut self, show: bool) {
        self.usage_show_system = show;
        self.rebuild_usage_rows();
    }
}

/// Display-ready row with every string pre-formatted. Built once per
/// report/sort/filter change (see `ManagerApp::rebuild_usage_rows`); the
/// `v_virtual_list` render closure only indexes into it — per-frame sorting
/// or formatting is what made the old 500-row page stutter.
pub struct UsageRow {
    pub name: SharedString,
    pub icon_path: Option<String>,
    pub active: String,
    pub sessions: String,
    pub last_seen: String,
    pub path: String,
}

pub fn build_usage_rows(v: &Value, show_system: bool, sort: UsageSort) -> Vec<UsageRow> {
    usage_rows(v, show_system, sort)
        .iter()
        .map(|row| UsageRow {
            name: SharedString::from(row_name(row)),
            icon_path: vopt(row, "iconPath").filter(|p| !p.is_empty()),
            active: fmt_duration(vnum(row, "foregroundMs")),
            sessions: vstr(row, "sessions"),
            last_seen: fmt_last_seen(row),
            path: vstr(row, "normalizedPath"),
        })
        .collect()
}

/// Rows ready for display: system processes (exe under %SystemRoot%) hidden
/// unless the toggle is on, then sorted per the header state.
pub fn usage_rows(v: &Value, show_system: bool, sort: UsageSort) -> Vec<Value> {
    let mut rows: Vec<Value> = varr(v, "topApps")
        .iter()
        .filter(|row| {
            show_system
                || !row
                    .get("isSystem")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .cloned()
        .collect();
    sort_rows(&mut rows, sort);
    rows
}

pub fn row_name(row: &Value) -> String {
    vopt(row, "displayName")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| vstr(row, "processName"))
}

fn sort_rows(rows: &mut [Value], sort: UsageSort) {
    let ord = |a: &Value, b: &Value| match sort.column {
        UsageColumn::Name => row_name(a)
            .to_lowercase()
            .cmp(&row_name(b).to_lowercase())
            .then_with(|| {
                vnum(a, "foregroundMs")
                    .total_cmp(&vnum(b, "foregroundMs"))
                    .reverse()
            }),
        UsageColumn::Active => vnum(a, "foregroundMs").total_cmp(&vnum(b, "foregroundMs")),
        UsageColumn::Sessions => vnum(a, "sessions").total_cmp(&vnum(b, "sessions")),
        // RFC3339 compares chronologically as text; missing sorts last either way.
        UsageColumn::LastSeen => vstr(a, "lastSeenAt").cmp(&vstr(b, "lastSeenAt")),
        UsageColumn::Path => vstr(a, "normalizedPath")
            .to_lowercase()
            .cmp(&vstr(b, "normalizedPath").to_lowercase()),
    };
    rows.sort_by(|a, b| {
        let ord = ord(a, b);
        if sort.ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}

/// UsageTable.vue fmtDuration parity: округление до минут, "N ч M мин".
pub fn fmt_duration(ms: f64) -> String {
    let total_minutes = (ms / 60_000.0).round() as i64;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours <= 0 {
        format!("{minutes} мин")
    } else if minutes == 0 {
        format!("{hours} ч")
    } else {
        format!("{hours} ч {minutes} мин")
    }
}

/// UsageTable.vue fmtDate parity: тот же момент времени, относительный формат
/// через kit fmt_ms ("2 ч. назад") — settings.rs convention для timestamps.
pub fn fmt_last_seen(entry: &Value) -> String {
    let Some(iso) = vopt(entry, "lastSeenAt").filter(|s| !s.is_empty()) else {
        return "—".into();
    };
    match iso_to_ms(&iso) {
        Some(ts) => fmt_ms((now_ms() - ts).max(0.0)),
        None => iso,
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// RFC3339 → unix ms. Покрывает формат usage tracker
/// (`Utc::now().to_rfc3339_opts(Millis, true)`): "YYYY-MM-DDTHH:MM:SS[.fff]"
/// + "Z" или "±HH:MM". Непарсабельное → None (view показывает raw ISO).
fn iso_to_ms(iso: &str) -> Option<f64> {
    if iso.len() < 19 || iso.as_bytes()[4] != b'-' {
        return None;
    }
    let year: i64 = iso.get(0..4)?.parse().ok()?;
    let month: u32 = iso.get(5..7)?.parse().ok()?;
    let day: u32 = iso.get(8..10)?.parse().ok()?;
    let hour: i64 = iso.get(11..13)?.parse().ok()?;
    let minute: i64 = iso.get(14..16)?.parse().ok()?;
    let second: i64 = iso.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut total = days_from_civil(year, month, day) * 86_400_000
        + hour * 3_600_000
        + minute * 60_000
        + second * 1_000;
    let rest = iso.get(19..)?;
    let zone = match rest.strip_prefix('.') {
        Some(frac) => frac.trim_start_matches(|c: char| c.is_ascii_digit()),
        None => rest,
    };
    match zone {
        "Z" | "" => {}
        z if z.starts_with('+') || z.starts_with('-') => {
            let sign: i64 = if z.starts_with('-') { -1 } else { 1 };
            let off_hours: i64 = z.get(1..3)?.parse().ok()?;
            let off_minutes: i64 = z.get(4..6)?.parse().ok()?;
            total -= sign * (off_hours * 3_600_000 + off_minutes * 60_000);
        }
        _ => return None,
    }
    Some(total as f64)
}

/// Howard Hinnant's days_from_civil — days since unix epoch.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * i64::from(mp) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::{fmt_duration, iso_to_ms, sort_rows, usage_rows, UsageColumn, UsageSort};
    use serde_json::{json, Value};

    fn row(name: &str, active_ms: i64, sessions: i64, last: &str, path: &str) -> Value {
        json!({
            "displayName": name,
            "processName": format!("{}.exe", name),
            "normalizedPath": path,
            "foregroundMs": active_ms,
            "sessions": sessions,
            "lastSeenAt": last,
        })
    }

    #[test]
    fn fmt_duration_matches_vue() {
        assert_eq!(fmt_duration(0.0), "0 мин");
        assert_eq!(fmt_duration(59_000.0), "1 мин");
        assert_eq!(fmt_duration(3_600_000.0), "1 ч");
        assert_eq!(fmt_duration(5_400_000.0), "1 ч 30 мин");
    }

    #[test]
    fn iso_to_ms_parses_utc_and_offsets() {
        assert_eq!(iso_to_ms("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(iso_to_ms("1970-01-01T00:00:00.123Z"), Some(0.0));
        assert_eq!(iso_to_ms("1970-01-01T03:00:00+03:00"), Some(0.0));
        assert_eq!(iso_to_ms("1969-12-31T21:00:00-03:00"), Some(0.0));
        assert_eq!(
            iso_to_ms("2026-06-09T14:32:11.456Z"),
            Some(1_781_015_531_000.0)
        );
        assert_eq!(iso_to_ms("not a date"), None);
    }

    #[test]
    fn default_sort_is_active_time_descending() {
        let v = json!({ "topApps": [
            row("Alpha", 10, 1, "2026-01-01T00:00:00Z", "c:\\a.exe"),
            row("Beta", 999, 1, "2026-01-01T00:00:00Z", "c:\\b.exe"),
            row("Gamma", 50, 1, "2026-01-01T00:00:00Z", "c:\\g.exe"),
        ]});
        let rows = usage_rows(&v, false, UsageSort::default());
        let names: Vec<String> = rows
            .iter()
            .map(|r| r["displayName"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["Beta", "Gamma", "Alpha"]);
    }

    #[test]
    fn sort_by_name_ascending_then_toggle_direction() {
        let mut rows = vec![
            row("Charlie", 1, 1, "", "c:\\c.exe"),
            row("Alpha", 2, 1, "", "c:\\a.exe"),
            row("Beta", 3, 1, "", "c:\\b.exe"),
        ];
        let mut sort = UsageSort::default();
        sort.toggled(UsageColumn::Name);
        sort_rows(&mut rows, sort);
        assert_eq!(rows[0]["displayName"], "Alpha");

        sort.toggled(UsageColumn::Name);
        sort_rows(&mut rows, sort);
        assert_eq!(rows[0]["displayName"], "Charlie");
    }

    #[test]
    fn sort_by_sessions_and_last_seen() {
        let mut rows = vec![
            row("A", 1, 1, "2026-01-01T00:00:00Z", "c:\\a.exe"),
            row("B", 1, 9, "2026-03-01T00:00:00Z", "c:\\b.exe"),
            row("C", 1, 5, "2026-02-01T00:00:00Z", "c:\\c.exe"),
        ];
        let mut sort = UsageSort::default();
        sort.toggled(UsageColumn::Sessions);
        sort_rows(&mut rows, sort);
        assert_eq!(rows[0]["displayName"], "B");
        sort.toggled(UsageColumn::LastSeen);
        sort_rows(&mut rows, sort);
        assert_eq!(rows[0]["displayName"], "B");
        assert_eq!(rows[2]["displayName"], "A");
    }

    #[test]
    fn system_rows_hidden_until_toggled() {
        let v = json!({ "topApps": [
            {
                "displayName": "Explorer",
                "processName": "explorer.exe",
                "normalizedPath": "c:\\windows\\explorer.exe",
                "isSystem": true,
                "foregroundMs": 500,
                "sessions": 1,
            },
            row("Beta", 10, 1, "", "c:\\b.exe"),
        ]});
        assert_eq!(usage_rows(&v, false, UsageSort::default()).len(), 1);
        assert_eq!(usage_rows(&v, true, UsageSort::default()).len(), 2);
    }
}
