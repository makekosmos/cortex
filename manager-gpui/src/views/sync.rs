//! Синхронизация — get_sync_snapshot, show_pairing_code,
//! connect_with_pairing_code, disconnect_peer (SyncView.vue parity).
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    // Only snapshots are loaded passively — both are non-elevating reads.
    // The pairing ticket is fetched by the explicit «Показать код для
    // подключения» button — on a loopback-bound Engine that request
    // escalates the bind, so the firewall prompt belongs to that click, not
    // to opening this tab (KOS-269).
    app.call("sync.snapshot", "get_sync_snapshot", json!({}));
    app.call("sync.privileged", "system.privileged.status", json!({}));
}

mod pairing;

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
            row(
                "Статус",
                format!("Транспорт: {}", transport_text(&vstr(v, "transport"))),
            )
            .child(badge(
                if running {
                    "Запущена"
                } else {
                    "Остановлена"
                },
                if running { SUCCESS() } else { WARN() },
            )),
        );
        el = el.child(kv("Устройство", vstr(device, "device_name")));
        el.into_any_element()
    }));

    col = col.child(pairing::ticket_card(app, cx));
    col = col.child(pairing::privileged_card(app, cx));

    let ticket_in = app.input(
        "sync.peer",
        "Код устройства для подключения…",
        false,
        window,
        cx,
    );
    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Подключить устройство"),
            )
            .child(div().text_size(px(12.)).text_color(c(MUTED_FG())).child(
                "При первом включении синхронизации Windows может один раз показать \
                         окно брандмауэра для Mundus Engine — нажмите «Разрешить доступ», \
                         чтобы устройства находили друг друга в локальной сети. \
                         Чтобы окно не появлялось снова после обновлений, включите \
                         «Расширенные права» кнопкой выше; без них Windows может \
                         спросить ещё раз после обновления приложения.",
            ))
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
            // Engine sends `device_id`/`device_name`; `peer_id` was never
            // populated, so the row showed an empty name and the disconnect
            // call carried an empty id.
            let id = vstr(p, "device_id");
            let name = {
                let n = vopt(p, "device_name")
                    .or_else(|| vopt(p, "name"))
                    .unwrap_or_default();
                if n.is_empty() {
                    "Устройство".to_string()
                } else {
                    n
                }
            };
            let peer_name = name.clone();
            el = el.child(
                row(name, peer_status_text(&vstr(p, "status"))).child(btn_id(
                    &format!("disc-{id}"),
                    "Отключить",
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Отключить устройство",
                            format!("«{peer_name}» будет удалено из списка синхронизации."),
                            "disconnect_peer",
                            json!({"device_id": id}),
                            cx,
                        );
                    }),
                )),
            );
        }
        el.into_any_element()
    }));
    col.into_any_element()
}

/// Peer `status` from the sync snapshot — mapped to Russian text in one place.
fn peer_status_text(status: &str) -> &'static str {
    match status {
        "online" => "В сети",
        "offline" => "Не в сети",
        _ => "Статус неизвестен",
    }
}

/// `transport` from the sync snapshot — the raw enum values stay off-screen.
fn transport_text(transport: &str) -> &'static str {
    match transport {
        "iroh" => "Iroh (P2P)",
        "relay" => "Через сервер",
        "lan" => "Локальная сеть",
        _ => "Неизвестно",
    }
}
