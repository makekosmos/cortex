//! Device rows and a separate pairing card, following Zeron Settings/Devices.
//! Engine remains the only owner of pairing and peer state.
use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::device_info::{peer_caption, peer_platform, Platform};
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

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
    app.call("sync.privileged", "system.privileged.status", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    page_stack()
        .child(section("Девайсы", "Ваши устройства и подключение по коду"))
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
                            .child(device_identity(
                                &app.local_device.name,
                                &app.local_device.caption(),
                            ))
                            .child(slot_or(app, "sync.snapshot", |snapshot| {
                                let running = vbool(snapshot, "running");
                                presence(
                                    if running {
                                        "Синхронизация активна"
                                    } else {
                                        "Синхронизация остановлена"
                                    },
                                    if running { SUCCESS() } else { WARN() },
                                )
                                .into_any_element()
                            })),
                    ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(section_label("Другие устройства"))
                .child(block().child(slot_or(app, "sync.snapshot", |snapshot| {
                    let peers = varr(snapshot, "peers");
                    let mut others = div().flex().flex_col();
                    if peers.is_empty() {
                        others =
                            others.child(block_row(true).child(
                                div().text_color(c(MUTED_FG())).child(
                                    "Других устройств пока нет. Подключите их по коду ниже.",
                                ),
                            ));
                    }
                    for (index, peer) in peers.iter().enumerate() {
                        let id = vstr(peer, "device_id");
                        let name = device_name(peer);
                        let confirmation_name = name.clone();
                        let online = vstr(peer, "status") == "online";
                        let can_disconnect = !id.is_empty() && !app.action_busy;
                        others = others.child(
                            block_row(index == 0)
                                .child(device_avatar(peer_platform(peer)))
                                .child(device_identity(&name, &peer_caption(peer)))
                                .child(presence(
                                    if online {
                                        "В сети"
                                    } else {
                                        peer_status_text(&vstr(peer, "status"))
                                    },
                                    if online { SUCCESS() } else { MUTED_FG() },
                                ))
                                .child(
                                    btn_id(
                                        &format!("disc-{id}"),
                                        "Отключить",
                                        cx.listener(move |this, _, _, cx| {
                                            this.ask_confirm("Отключить устройство",
                            format!("«{confirmation_name}» будет удалено из списка синхронизации."),
                            "disconnect_peer", json!({"device_id": id}), cx);
                                        }),
                                    )
                                    .disabled(!can_disconnect),
                                ),
                        );
                    }
                    others.into_any_element()
                }))),
        )
        .child(pairing::render(app, window, cx))
        .into_any_element()
}

fn device_name(device: &Value) -> String {
    vopt(device, "device_name")
        .or_else(|| vopt(device, "name"))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Устройство".into())
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
        .text_size(px(12.))
        .text_color(c(color))
        .child(div().size(px(6.)).rounded_full().bg(c(color)))
        .child(label.to_owned())
}

pub(super) fn section_label(label: &str) -> Div {
    div()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(MUTED_FG()))
        .child(label.to_owned())
}

pub(super) fn block() -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded_xl()
        .overflow_hidden()
        .bg(fade(FG(), 0.025))
        .border_1()
        .border_color(c(BORDER()))
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
