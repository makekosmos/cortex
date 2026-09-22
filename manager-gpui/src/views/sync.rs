//! Синхронизация — get_sync_snapshot, get_own_iroh_ticket,
//! connect_with_pairing_code, disconnect_peer (SyncView.vue parity).
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("sync.snapshot", "get_sync_snapshot", json!({}));
    app.call("sync.ticket", "get_own_iroh_ticket", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Синхронизация", "Устройства и связи"));

    col = col.child(slot_or(app, "sync.snapshot", |v| {
        let running = vbool(v, "running");
        let device = vget(v, "local_device");
        let mut el = card();
        el = el.child(
            row("Статус", format!("Транспорт: {}", vstr(v, "transport"))).child(badge(
                if running {
                    "Запущена"
                } else {
                    "Остановлена"
                },
                if running { SUCCESS() } else { WARN() },
            )),
        );
        el = el.child(kv("Устройство", vstr(device, "device_name")));
        el = el.child(kv("ID устройства", vstr(device, "device_id")));
        el.into_any_element()
    }));

    col = col.child(slot_or(app, "sync.ticket", |v| {
        let ticket = if v.is_null() {
            String::new()
        } else {
            vstr(v, "ticket")
        };
        let code = if ticket.is_empty() {
            vstr(v, "code")
        } else {
            ticket
        };
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child("Ваш код подключения"),
        );
        if code.is_empty() {
            el = el.child(empty("Код недоступен — синхронизация не готова"));
        } else {
            el = el.child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(code),
            );
        }
        el.into_any_element()
    }));

    let ticket_in = app.input("sync.peer", "Код устройства для подключения…", window, cx);
    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Подключить устройство"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&ticket_in)))
                    .child(btn(
                        "sync-connect",
                        "Подключить",
                        true,
                        cx,
                        |this, cx| {
                            let code = this.input_value("sync.peer", cx);
                            if !code.is_empty() {
                                this.action(
                                    "connect_with_pairing_code",
                                    json!({"ticket": code, "pairing_code": code}),
                                );
                            }
                        },
                    )),
            ),
    );

    col = col.child(slot_or(app, "sync.snapshot", |v| {
        let peers = varr(v, "peers");
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child(format!("Связанные устройства ({})", peers.len())),
        );
        if peers.is_empty() {
            el = el.child(empty("Нет связанных устройств"));
        }
        for p in peers {
            let id = vstr(p, "peer_id");
            let name = {
                let n = vopt(p, "device_name")
                    .or_else(|| vopt(p, "name"))
                    .unwrap_or_default();
                if n.is_empty() {
                    id.clone()
                } else {
                    n
                }
            };
            let status = vopt(p, "status").unwrap_or_else(|| "unknown".into());
            el = el.child(row(name, format!("{id} · {status}")).child(btn_id(
                &format!("disc-{id}"),
                "Отключить",
                cx.listener(move |this, _, _, cx| {
                    this.ask_confirm(
                        "Отключить устройство",
                        "Устройство будет удалено из списка синхронизации.",
                        "disconnect_peer",
                        json!({"peer_id": id}),
                        cx,
                    );
                }),
            )));
        }
        el.into_any_element()
    }));
    col.into_any_element()
}
