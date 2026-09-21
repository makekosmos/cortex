//! View primitives + JSON accessors: section/card/row/kv/badge/btn/toggle/
//! empty/slot_or and the serde_json::Value getters every view uses.
use ::gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::switch::Switch;
use serde_json::Value;

use crate::app::ManagerApp;
use crate::theme::*;

// --- JSON accessors ---------------------------------------------------------

pub fn vget<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap_or(&Value::Null)
}

pub fn vstr(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| {
            x.as_str()
                .map(str::to_string)
                .or_else(|| Some(x.to_string()))
        })
        .unwrap_or_default()
}

pub fn vopt(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

pub fn vbool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub fn vnum(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn varr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn fmt_bytes(n: f64) -> String {
    if n >= 1_073_741_824.0 {
        format!("{:.1} ГБ", n / 1_073_741_824.0)
    } else if n >= 1_048_576.0 {
        format!("{:.1} МБ", n / 1_048_576.0)
    } else if n >= 1024.0 {
        format!("{:.0} КБ", n / 1024.0)
    } else {
        format!("{:.0} Б", n)
    }
}

pub fn fmt_ms(ms: f64) -> String {
    let secs = ms / 1000.0;
    let days = (secs / 86400.0).floor();
    let h = (secs % 86400.0) / 3600.0;
    let m = (secs % 3600.0) / 60.0;
    if days >= 1.0 {
        format!("{days:.0} дн. назад")
    } else if h >= 1.0 {
        format!("{h:.0} ч. назад")
    } else if m >= 1.0 {
        format!("{m:.0} мин. назад")
    } else {
        "только что".into()
    }
}

// --- View primitives --------------------------------------------------------

pub fn section(title: &str, hint: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(c(MUTED_FG))
                .child(hint.to_string()),
        )
}

pub fn card() -> Div {
    div()
        .w_full()
        .rounded_lg()
        .border_1()
        .border_color(c(BORDER))
        .bg(c(CARD))
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
}

pub fn row(label: impl Into<String>, sub: impl Into<String>) -> Div {
    div()
        .w_full()
        .min_h_10()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_sm().child(label.into()))
                .child(div().text_xs().text_color(c(MUTED_FG)).child(sub.into())),
        )
}

pub fn kv(key: &str, value: impl Into<String>) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap_2()
        .child(
            div()
                .w(px(180.))
                .flex_none()
                .text_sm()
                .text_color(c(MUTED_FG))
                .child(key.to_string()),
        )
        .child(div().flex_1().text_sm().child(value.into()))
}

pub fn badge(text: impl Into<String>, color: u32) -> impl IntoElement {
    div()
        .px_2()
        .py_0p5()
        .rounded_full()
        .bg(fade(color, 0.15))
        .text_xs()
        .text_color(c(color))
        .child(text.into())
}

pub fn btn(
    id: &'static str,
    label: &'static str,
    primary: bool,
    cx: &mut Context<ManagerApp>,
    on_click: impl Fn(&mut ManagerApp, &mut Context<ManagerApp>) + 'static,
) -> Button {
    let b = Button::new(id)
        .label(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            on_click(this, cx);
            cx.notify();
        }));
    if primary {
        b.primary()
    } else {
        b.ghost()
    }
}

/// Button whose id must be dynamic (per-row actions like disconnect/uninstall).
/// Pass `cx.listener(...)` — it adapts the entity handler to the App callback.
pub fn btn_id(
    id: &str,
    label: &'static str,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(SharedString::from(id.to_string()))
        .ghost()
        .label(label)
        .on_click(listener)
}

pub fn toggle(
    id: &'static str,
    checked: bool,
    cx: &mut Context<ManagerApp>,
    on_click: impl Fn(&mut ManagerApp, bool, &mut Context<ManagerApp>) + 'static,
) -> Switch {
    Switch::new(id)
        .checked(checked)
        .on_click(cx.listener(move |this, checked, _, cx| {
            on_click(this, *checked, cx);
            cx.notify();
        }))
}

pub fn empty(text: &str) -> impl IntoElement {
    div()
        .p_6()
        .text_sm()
        .text_color(c(MUTED_FG))
        .child(text.to_string())
}

pub fn slot_or<F>(app: &ManagerApp, slot: &str, render: F) -> AnyElement
where
    F: FnOnce(&Value) -> AnyElement,
{
    match app.slot(slot) {
        Some(crate::app::Slot::Ready(v)) => render(v),
        Some(crate::app::Slot::Failed(e)) => div()
            .text_sm()
            .text_color(c(DESTRUCTIVE))
            .child(e.clone())
            .into_any_element(),
        _ => div()
            .text_sm()
            .text_color(c(MUTED_FG))
            .child("Загрузка…")
            .into_any_element(),
    }
}
