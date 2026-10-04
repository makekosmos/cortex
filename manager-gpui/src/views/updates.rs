use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("upd.mundus", "updater.status", json!({}));
}

/// Compact updates section on the About page, matching its diagnostics group.
pub fn render(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let status = app.data("upd.mundus");
    let state = vstr(&status, "state");
    let current = crate::async_fields::field_text(app.slots.get("upd.mundus"), |value| {
        crate::device_info::version_label(&vstr(value, "currentVersion"), &vstr(value, "channel"))
    });
    let next = vstr(&status, "newVersion");
    let busy = app.action_busy || matches!(state.as_str(), "checking" | "downloading");
    let button = if matches!(state.as_str(), "available" | "downloading" | "downloaded") {
        crate::button::button("mundus-install", crate::button::ButtonKind::Success)
            // Keep the semantic success tint when the installer is unavailable;
            // the component's default disabled style replaces it with gray.
            .bg(fade(SUCCESS(), 0.20))
            .text_color(c(SUCCESS()))
            .hover(|style| style.bg(fade(SUCCESS(), 0.28)))
            .border_0()
            .label("Установить обновление")
            .disabled(busy || status.get("canInstall").and_then(Value::as_bool) == Some(false))
            .on_click(cx.listener(|this, _, _, cx| {
                let state = vstr(&this.data("upd.mundus"), "state");
                this.action(
                    if state == "downloaded" {
                        "updater.install"
                    } else {
                        "updater.download"
                    },
                    json!({}),
                );
                cx.notify();
            }))
    } else {
        crate::button::secondary("mundus-check")
            .label(if state == "checking" {
                "Проверяем…"
            } else {
                "Проверить обновления"
            })
            .disabled(busy)
            .on_click(cx.listener(|this, _, _, cx| {
                this.action("updater.check", json!({}));
                cx.notify();
            }))
    };
    let version = if next.is_empty() {
        current
    } else {
        format!("{current} → {next}")
    };
    let mut content = card()
        .id("about-updates-card")
        .debug_selector(|| "about-updates-card".into())
        .child(
            div()
                .w_full()
                .min_w_0()
                .min_h(px(36.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(crate::theme::ui_px(13.))
                                .line_height(crate::theme::ui_px(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Обновления Mundus"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .text_size(crate::theme::ui_px(12.))
                                .line_height(crate::theme::ui_px(16.))
                                .text_color(c(MUTED_FG()))
                                .child(
                                    gpui_component::Icon::default()
                                        .path("icons/mundus.svg")
                                        .size(px(12.))
                                        .text_color(c(ACCENT())),
                                )
                                .child(version),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .id("about-updates-action")
                        .debug_selector(|| "about-updates-action".into())
                        .child(button),
                ),
        );

    if state == "downloading" {
        let percent = vnum(&status, "percent").clamp(0.0, 100.0);
        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(format!("Скачивание {percent:.0}%"))
                .child(
                    div()
                        .h_1()
                        .w_full()
                        .rounded_full()
                        .bg(fade(FG(), 0.12))
                        .child(
                            div()
                                .h_full()
                                .w(relative(percent as f32 / 100.0))
                                .rounded_full()
                                .bg(c(ACCENT())),
                        ),
                ),
        );
    }
    section_group()
        .child(
            div()
                .text_size(crate::theme::ui_px(13.))
                .child("Обновления"),
        )
        .child(content)
        .into_any_element()
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
