use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("upd.mundus", "updater.status", json!({}));
    app.call("upd.catalog", "store.catalog", json!({}));
    app.call("upd.installed", "packages.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    page_stack()
        .child(section("Обновления", "Cortex и приложения"))
        .child(render_mundus(app, cx))
        .child(
            card().child(row("Каталог", "Свежесть списка пакетов и цен").child(btn(
                "upd-refresh",
                "Проверить пакеты",
                true,
                cx,
                |this, _| {
                    this.action("store.refresh", json!({}));
                    this.action("packages.refresh_catalog", json!({}));
                },
            ))),
        )
        .child(render_package_updates(app, cx))
        .into_any_element()
}

fn render_mundus(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let status = app.data("upd.mundus");
    let state = vstr(&status, "state");
    let current = crate::async_fields::field_text(app.slots.get("upd.mundus"), |value| {
        crate::device_info::version_label(&vstr(value, "currentVersion"), &vstr(value, "channel"))
    });
    let next = vstr(&status, "newVersion");
    let detail = crate::async_fields::field_text(app.slots.get("upd.mundus"), mundus_status);
    let mut content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(row("Cortex", detail).child(badge(&current, MUTED_FG())));

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
    if !next.is_empty() {
        content = content.child(kv("Версии", format!("{current} → {next}")));
    }

    content = content.child(crate::async_fields::field_row(
        app,
        "upd.mundus",
        "Установка",
        |value| match value.get("canInstall").and_then(Value::as_bool) {
            Some(true) => "Автоматически".into(),
            Some(false) => vstr(value, "installUnavailableReason"),
            None => String::new(),
        },
    ));
    let button = if state == "downloaded" {
        btn(
            "mundus-install",
            "Обновить и перезапустить",
            true,
            cx,
            |this, _| this.action("updater.install", json!({})),
        )
        .disabled(status.get("canInstall").and_then(Value::as_bool) == Some(false))
    } else {
        btn(
            "mundus-check",
            "Проверить обновления",
            !matches!(state.as_str(), "checking" | "downloading"),
            cx,
            |this, _| this.action("updater.check", json!({})),
        )
    };
    card()
        .id("updates-product-card")
        .debug_selector(|| "updates-product-card".into())
        .child(content.child(button))
        .into_any_element()
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;

fn mundus_status(status: &Value) -> String {
    match vstr(status, "state").as_str() {
        "idle" => "Готов к проверке".into(),
        "checking" => "Проверяем новую версию…".into(),
        "available" => "Доступна новая версия".into(),
        "downloading" => "Новая версия загружается".into(),
        "downloaded" => "Обновление готово к установке".into(),
        "not-available" => "Установлена актуальная версия".into(),
        "error" => format!("Не удалось проверить: {}", vstr(status, "message")),
        _ => "Статус недоступен".into(),
    }
}

fn render_package_updates(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let catalog = app.data("upd.catalog");
    let installed = app.data("upd.installed");
    if catalog.is_null() && installed.is_null() {
        return empty("Загрузка…").into_any_element();
    }
    let mut updates = Vec::new();
    for package in varr(&installed, "packages") {
        let id = vstr(package, "id");
        let current = vstr(package, "version");
        let latest = varr(&catalog, "listings")
            .iter()
            .find(|listing| vstr(listing, "id") == id)
            .map(|listing| vstr(vget(listing, "distribution"), "version"))
            .unwrap_or_default();
        if !latest.is_empty() && latest != current {
            let name = vopt(package, "name")
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Пакет".into());
            updates.push((id, name, vopt(package, "icon_path"), current, latest));
        }
    }
    let mut element = card().child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child(format!("Доступные обновления ({})", updates.len())),
    );
    if updates.is_empty() {
        element = element.child(empty("Все пакеты актуальны"));
    }
    for (id, name, icon_path, current, latest) in updates {
        let package_id = id.clone();
        element = element.child(
            entry_row(icon_file(icon_path), name, format!("{current} → {latest}")).child(btn_id(
                &format!("upd-{id}"),
                "Обновить",
                cx.listener(move |this, _, _, cx| {
                    this.action("packages.install", json!({"package_id": package_id}));
                    cx.notify();
                }),
            )),
        );
    }
    element.into_any_element()
}
