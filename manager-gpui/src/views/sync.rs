//! Device rows and a separate pairing card, following Zeron Settings/Devices.
//! Engine remains the only owner of pairing and peer state.
use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::device_info::Platform;
use crate::theme::*;
use crate::widgets::*;

mod devices;
mod pairing;
#[cfg(test)]
mod tests;

pub fn load(app: &mut ManagerApp) {
    // Only snapshots are loaded passively — both are non-elevating reads.
    // The pairing ticket is fetched by the explicit «Показать код для
    // подключения» button — on a loopback-bound Engine that request
    // escalates the bind, so the firewall prompt belongs to that click, not
    // to opening this tab (KOS-269).
    app.call("sync.snapshot", "get_sync_snapshot", json!({}));
    app.call("sync.registry", "device_registry.snapshot", json!({}));
    app.call("sync.privileged", "system.privileged.status", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let registry = app.data("sync.registry");
    let sync = app.data("sync.snapshot");
    let local_id = vstr(&registry, "localDeviceId");
    let local_name = devices::local_name(&registry, &local_id, &app.local_device.name);
    let (local_status, local_status_color) = devices::local_status(&registry, &sync);
    let system_name = app.local_device.name.clone();
    let others = devices::render_others(app, &registry, &sync, &system_name, window, cx);

    page_stack()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(section_label("Это устройство"))
                .child(
                    block().child(
                        block_row(true)
                            .child(device_avatar(app.local_device.platform))
                            .child(device_identity(&local_name, &app.local_device.caption()))
                            .child(presence(local_status, local_status_color))
                            .child(devices::render_local_name_editor(
                                app,
                                &local_id,
                                &local_name,
                                window,
                                cx,
                            )),
                    ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(section_label("Другие устройства"))
                .child(block().child(others)),
        )
        .child(pairing::render(app, window, cx))
        .into_any_element()
}

fn device_identity(name: &str, detail: &str) -> Div {
    row_copy(name, detail)
}

fn device_avatar(platform: Platform) -> Div {
    div()
        .size(px(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(fade(FG(), 0.04))
        .child(
            svg()
                .path(platform.icon())
                .size(px(22.))
                .text_color(fade(FG(), 0.7)),
        )
}

fn presence(label: &str, color: u32) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .text_size(crate::theme::ui_px(12.))
        .text_color(c(color))
        .child(div().size(px(6.)).rounded_full().bg(c(color)))
        .child(label.to_owned())
}

pub(super) fn section_label(label: &str) -> Div {
    div()
        .text_size(crate::theme::ui_px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(MUTED_FG()))
        .child(label.to_owned())
}

pub(super) fn block() -> Div {
    card().px(px(0.)).py(px(0.)).gap(px(0.))
}

pub(super) fn block_row(first: bool) -> Div {
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .when(!first, |row| row.border_t_1().border_color(c(BORDER())))
}

fn peer_status_text(status: &str) -> &'static str {
    match status {
        "online" => "В сети",
        "offline" => "Не в сети",
        _ => "Статус неизвестен",
    }
}

pub(super) fn pairing_code(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| vopt(value, "ticket"))
        .or_else(|| vopt(value, "code"))
        .unwrap_or_default()
        .trim()
        .to_owned()
}
