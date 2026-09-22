//! Обновления — desktop update state is Host-owned (electron-updater); GPUI
//! shows package updates via store.catalog + packages.list comparison and the
//! same refresh actions as UpdatesView.vue.
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("upd.catalog", "store.catalog", json!({}));
    app.call("upd.installed", "packages.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Обновления", "Kosmos Desktop и приложения"));

    col = col.child(
        card().child(
            row(
                "Kosmos Desktop",
                "Обновления оболочки доставляет Kosmos Host (Electron). Здесь отображаются обновления пакетов Engine.",
            )
            .child(badge("Host", MUTED_FG())),
        ),
    );

    col = col.child(
        card().child(row("Каталог", "Свежесть списка пакетов и цен").child(btn(
            "upd-refresh",
            "Проверить обновления",
            true,
            cx,
            |this, _| {
                this.action("store.refresh", json!({}));
                this.action("packages.refresh_catalog", json!({}));
            },
        ))),
    );

    col = col.child(render_updates(app, cx));
    col.into_any_element()
}

fn render_updates(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let catalog = app.data("upd.catalog");
    let installed = app.data("upd.installed");
    if catalog.is_null() && installed.is_null() {
        return empty("Загрузка…").into_any_element();
    }
    let mut updates: Vec<(String, String, String)> = vec![];
    for p in varr(&installed, "packages") {
        let id = vstr(p, "id");
        let current = vstr(p, "version");
        let latest = varr(&catalog, "listings")
            .iter()
            .find(|l| vstr(l, "id") == id)
            .map(|l| vstr(l, "version"))
            .unwrap_or_default();
        if !latest.is_empty() && latest != current {
            updates.push((id, current, latest));
        }
    }
    let mut el = card();
    el = el.child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child(format!("Доступные обновления ({})", updates.len())),
    );
    if updates.is_empty() {
        el = el.child(empty("Все пакеты актуальны"));
    }
    for (id, current, latest) in updates {
        let pid = id.clone();
        el = el.child(
            row(id.clone(), format!("{current} → {latest}")).child(btn_id(
                &format!("upd-{id}"),
                "Обновить",
                {
                    cx.listener(move |this, _, _, cx| {
                        this.action("packages.install", json!({"package_id": pid}));
                        cx.notify();
                    })
                },
            )),
        );
    }
    el.into_any_element()
}
