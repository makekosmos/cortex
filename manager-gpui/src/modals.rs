//! Modal layers: destructive-action confirm and pairing prompt.
//! store listing detail.
use crate::button;
use ::gpui::{prelude::*, *};
use serde_json::Value;

use crate::app::{Confirm, ManagerApp};
use crate::theme::*;
use mundus_gpui_kit::fields::vopt;

pub fn render_confirm(confirm: &Confirm, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    div()
        .absolute()
        .size_full()
        .bg(fade(0x000000, 0.5))
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .w_full()
                .max_w(px(420.))
                .mx(px(24.))
                .p(px(crate::page_layout::INSET))
                .rounded(px(12.))
                .bg(c(POPOVER()))
                .border_1()
                .border_color(c(BORDER()))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(crate::theme::ui_px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(confirm.title.clone()),
                )
                .child(
                    div()
                        .text_size(crate::theme::ui_px(13.))
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

/// KOS-369: an unknown device that entered our pairing code waits on an
/// explicit «Принять / Отклонить». The dialog is driven entirely by the
/// `incoming_pairing_requests` list in the sync snapshot — it appears on any
/// view and disappears only after a decision (or when the initiator cancels,
/// which drops the request engine-side).
pub fn render_pairing_prompt(request: &Value, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let device_id = vopt(request, "device_id").unwrap_or_default();
    let name = vopt(request, "device_name")
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Устройство".into());
    let platform =
        crate::device_info::Platform::parse(&vopt(request, "platform").unwrap_or_default())
            .label()
            .map(str::to_owned);
    let description = match &platform {
        Some(os) => format!("«{name}» ({os}) хочет подключиться к этому устройству."),
        None => format!("«{name}» хочет подключиться к этому устройству."),
    };
    let decline_id = device_id.clone();
    div()
        .absolute()
        .size_full()
        .bg(fade(0x000000, 0.5))
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .w_full()
                .max_w(px(420.))
                .mx(px(24.))
                .p(px(crate::page_layout::INSET))
                .rounded(px(12.))
                .bg(c(POPOVER()))
                .border_1()
                .border_color(c(BORDER()))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(crate::theme::ui_px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Подключение нового устройства"),
                )
                .child(
                    div()
                        .text_size(crate::theme::ui_px(13.))
                        .text_color(c(MUTED_FG()))
                        .child(description),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            button::ghost("pairing-decline")
                                .label("Отклонить")
                                // The a11y tree (and assistive tech) only sees
                                // interactive nodes — carry the requester in
                                // the button names.
                                .accessibility_label(format!(
                                    "Отклонить подключение от «{name}»{}",
                                    platform
                                        .as_deref()
                                        .map(|os| format!(" ({os})"))
                                        .unwrap_or_default()
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.action(
                                        "sync.pair.decline",
                                        serde_json::json!({"device_id": decline_id}),
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            button::primary("pairing-accept")
                                .label("Принять")
                                .accessibility_label(format!(
                                    "Принять подключение от «{name}»{}",
                                    platform
                                        .as_deref()
                                        .map(|os| format!(" ({os})"))
                                        .unwrap_or_default()
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.action(
                                        "sync.pair.accept",
                                        serde_json::json!({"device_id": device_id}),
                                    );
                                    cx.notify();
                                })),
                        ),
                ),
        )
}
