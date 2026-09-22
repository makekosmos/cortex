//! Интеграции — integrations.list/set_credential/clear_credential/sync_now +
//! login_contract/login_complete (ConnectionsView.vue parity).
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("conn.list", "integrations.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Интеграции", "Источники данных"));

    let snapshot = app.data("conn.list");
    let mut el = div().flex().flex_col().gap_3();
    let providers = varr(&snapshot, "providers").to_vec();
    if providers.is_empty() {
        el = el.child(card().child(empty("Нет доступных интеграций")));
    }
    for p in &providers {
        el = el.child(render_provider(app, p, window, cx));
    }
    col = col.child(el);
    col.into_any_element()
}

fn render_provider(
    app: &mut ManagerApp,
    p: &serde_json::Value,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let id = vstr(p, "id");
    let name = vopt(p, "name").unwrap_or_else(|| id.clone());
    let connected = vbool(p, "connected") || vbool(p, "has_credential");
    let status = vopt(p, "status").unwrap_or_else(|| "unknown".into());
    let last_sync = vopt(p, "last_sync_at").unwrap_or_else(|| "никогда".into());

    let cred_key = format!("conn.cred.{id}");
    let input = app.input(&cred_key, "API-ключ / токен…", window, cx);

    let sid = id.clone();
    let cid = id.clone();
    let yid = id.clone();
    let card_el = card()
        .child(
            row(
                name,
                format!("{status} · последняя синхронизация: {last_sync}"),
            )
            .child(badge(
                if connected {
                    "Подключена"
                } else {
                    "Не подключена"
                },
                if connected { SUCCESS() } else { MUTED_FG() },
            ))
            .child(btn_id(
                &format!("sync-{id}"),
                "Синхронизировать",
                {
                    cx.listener(move |this, _, _, cx| {
                        this.action(
                            "integrations.sync_now",
                            json!({"provider": sid, "provider_id": sid}),
                        );
                        cx.notify();
                    })
                },
            )),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(div().flex_1().child(Input::new(&input)))
                .child(btn_id(
                    &format!("set-{id}"),
                    "Сохранить ключ",
                    {
                        cx.listener(move |this, _, _, cx| {
                            let key = this.input_value(&cid, cx);
                            if !key.is_empty() {
                                this.action(
                                    "integrations.set_credential",
                                    json!({"provider": cid, "provider_id": cid, "credential": key}),
                                );
                            }
                        })
                    },
                ))
                .child(btn_id(&format!("clr-{id}"), "Удалить ключ", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Удалить ключ",
                            "Сохранённые учётные данные интеграции будут удалены.",
                            "integrations.clear_credential",
                            json!({"provider": yid, "provider_id": yid}),
                            cx,
                        );
                    })
                })),
        );
    card_el.into_any_element()
}
