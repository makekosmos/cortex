//! Затреканное время — UsageTable.vue / store.ts::loadUsageRows parity.
//! `get_usage_analytics` is forwarded by Engine to ark-core-rpc; per-row icons
//! are resolved worker-side via `app_index.icon_path` (`kosmos-icon://` is an
//! Electron protocol and has no GPUI equivalent).
use ::gpui::{prelude::*, *};
use serde_json::Value;

use crate::app::ManagerApp;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.usage_report("usage.report");
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    _cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section(
        "Затреканное время",
        "Время работы приложений по данным usage tracker",
    ));
    col = col.child(slot_or(app, "usage.report", |v| {
        let rows = varr(v, "topApps");
        let mut table = card();
        table = table.child(header_row());
        if rows.is_empty() {
            table = table.child(empty("Затреканное время не найдено"));
        }
        for entry in rows.iter().take(500) {
            table = table.child(usage_row(entry));
        }
        table.into_any_element()
    }));
    col.into_any_element()
}

fn head_cell(text: &str, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(MUTED_FG()))
        .whitespace_nowrap()
        .overflow_hidden()
        .child(text.to_string())
}

fn head_grow(text: &str) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(MUTED_FG()))
        .whitespace_nowrap()
        .overflow_hidden()
        .child(text.to_string())
}

fn header_row() -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .pb_2()
        .border_b_1()
        .border_color(c(BORDER()))
        .child(div().w(px(20.)).flex_none())
        .child(head_grow("Приложение"))
        .child(head_cell("Всего", 92.))
        .child(head_cell("Активно", 92.))
        .child(head_cell("Запусков", 76.))
        .child(head_cell("Последний запуск", 150.))
        .child(head_grow("Путь"))
}

fn metric(text: String, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .text_size(px(13.))
        .whitespace_nowrap()
        .overflow_hidden()
        .child(text)
}

fn usage_row(entry: &Value) -> Div {
    let name = vopt(entry, "displayName")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| vstr(entry, "processName"));
    let icon_path = vopt(entry, "iconPath").filter(|p| !p.is_empty());

    // 20px icon slot: cached PNG via app_index.icon_path, letter badge otherwise
    // (Vue renders an <img> that hides itself on error).
    let mut icon = div()
        .w(px(20.))
        .h(px(20.))
        .flex_none()
        .rounded_md()
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .bg(fade(FG(), 0.06))
        .text_size(px(11.))
        .text_color(c(MUTED_FG()));
    icon = match icon_path {
        Some(path) => icon.child(img(std::path::PathBuf::from(path)).size(px(20.))),
        None => icon.child(
            name.chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
        ),
    };

    div()
        .w_full()
        .min_h_10()
        .flex()
        .items_center()
        .gap_3()
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap()
                .text_ellipsis()
                .overflow_hidden()
                .child(name),
        )
        .child(metric(fmt_duration(vnum(entry, "runtimeMs")), 92.))
        .child(metric(fmt_duration(vnum(entry, "foregroundMs")), 92.))
        .child(metric(vstr(entry, "sessions"), 76.))
        .child(metric(fmt_last_seen(entry), 150.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .text_color(fade(FG(), 0.65))
                .whitespace_nowrap()
                .text_ellipsis()
                .overflow_hidden()
                .child(vstr(entry, "normalizedPath")),
        )
}

/// UsageTable.vue fmtDuration parity: округление до минут, "N ч M мин".
fn fmt_duration(ms: f64) -> String {
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
fn fmt_last_seen(entry: &Value) -> String {
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
    use super::{fmt_duration, iso_to_ms};

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
}
