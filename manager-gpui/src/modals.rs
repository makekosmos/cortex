//! Modal layers: destructive-action confirm, package disclosure consent,
//! store listing detail.
use ::gpui::{prelude::*, *};
use gpui_component::scroll::ScrollableElement;
use imago_gpui::button;
use serde_json::Value;

use crate::app::{Confirm, ManagerApp};
use crate::fields::vopt;
use crate::theme::*;

pub fn render_confirm(confirm: &Confirm, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    div()
        .absolute()
        .size_full()
        .bg(fade(0x000000, 0.5))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(420.))
                .p_5()
                .rounded_lg()
                .bg(c(POPOVER()))
                .border_1()
                .border_color(c(BORDER()))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(confirm.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(c(MUTED_FG()))
                        .child(confirm.body.clone()),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            button::ghost("cancel")
                                .label("Отмена")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.confirm = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            button::danger("ok")
                                .label("Подтвердить")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(c) = this.confirm.take() {
                                        this.action(c.op, c.params);
                                    }
                                    cx.notify();
                                })),
                        ),
                ),
        )
}

pub fn render_overlay(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let (title, body) = if let Some(d) = &app.disclosure {
        (
            "Требуемые разрешения".to_string(),
            serde_json::to_string_pretty(d).unwrap_or_default(),
        )
    } else {
        let d = app.detail.clone().unwrap_or(Value::Null);
        (
            vopt(&d, "name").unwrap_or_else(|| "Пакет".into()),
            serde_json::to_string_pretty(&d).unwrap_or_default(),
        )
    };
    div()
        .absolute()
        .size_full()
        .bg(fade(0x000000, 0.5))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.detail = None;
                this.disclosure = None;
                cx.notify();
            }),
        )
        .child(
            div()
                .id("overlay-card")
                .w(px(520.))
                .max_h(px(480.))
                .p_5()
                .rounded_lg()
                .bg(c(POPOVER()))
                .border_1()
                .border_color(c(BORDER()))
                .flex()
                .flex_col()
                .gap_3()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .text_size(px(12.))
                        .text_color(c(MUTED_FG()))
                        .child(body),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            button::ghost("close")
                                .label("Закрыть")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.detail = None;
                                    this.disclosure = None;
                                    this.pending_install = None;
                                    cx.notify();
                                })),
                        )
                        .when(app.pending_install.is_some(), |d| {
                            d.child(
                                button::primary("consent-install")
                                    .label("Установить")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(pid) = this.pending_install.take() {
                                            this.disclosure = None;
                                            this.action(
                                                "packages.install",
                                                serde_json::json!({"package_id": pid}),
                                            );
                                        }
                                        cx.notify();
                                    })),
                            )
                        }),
                ),
        )
}
