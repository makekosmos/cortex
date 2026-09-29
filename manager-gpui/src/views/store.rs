//! Маркетплейс — store.catalog/refresh/external_url + packages.list/install/
//! set_enabled/uninstall/trust_status/disclosure (StoreView+PackagesView parity).
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::views::StoreTab;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("store.catalog", "store.catalog", json!({}));
    app.call("store.installed", "packages.list", json!({}));
    app.call("store.trust", "packages.trust_status", json!({}));
    // KOS-265 native GPUI apps — separate install path from .kspkg packages.
    app.call("store.apps", "apps.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let _ = window;
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Маркетплейс", "Приложения и интеграции"));

    // Tab row: Каталог / Установленные
    let mut tabs = div().flex().gap_2();
    for (tab, label) in [
        (StoreTab::Catalog, "Каталог"),
        (StoreTab::Installed, "Установленные"),
    ] {
        let active = app.store_tab == tab;
        tabs = tabs.child(
            div()
                .id(SharedString::from(format!("tab-{label}")))
                .px_3()
                .h_8()
                .flex()
                .items_center()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(13.))
                .when(active, |d| d.bg(fade(ACCENT(), 0.18)))
                .when(!active, |d| d.hover(|s| s.bg(fade(FG(), 0.06))))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.store_tab = tab;
                    cx.notify();
                }))
                .role(Role::Tab)
                .aria_label(label)
                .aria_selected(active),
        );
    }
    tabs = tabs
        .child(div().flex_1())
        .child(btn(
            "store-ext",
            "Открыть в браузере",
            false,
            cx,
            |this, _| {
                this.call("store.ext", "store.external_url", json!({}));
            },
        ))
        .child(btn(
            "store-refresh",
            "Обновить каталог",
            false,
            cx,
            |this, _| {
                this.action("store.refresh", json!({}));
                this.action("packages.refresh_catalog", json!({}));
                // Native app rows re-check GitHub Releases past the TTL —
                // as a background refresh, so a successful reply can't clear
                // the catalog-refresh error banner.
                this.refresh("store.apps", "apps.list", json!({ "refresh": true }));
            },
        ));
    col = col.child(tabs);

    col = col.child(slot_or(app, "store.trust", |v| {
        let trust = vget(v, "trust");
        let configured = vbool(trust, "configured");
        let mut el = card();
        el = el.child(
            row("Доверие каталога", "Ключи выпуска и отзывы").child(badge(
                if configured {
                    "Настроено"
                } else {
                    "Не настроено"
                },
                if configured { SUCCESS() } else { WARN() },
            )),
        );
        el = el.child(kv("Доверенные ключи", vstr(trust, "trusted_release_keys")));
        el = el.child(kv("Отозванные пакеты", vstr(trust, "revoked_packages")));
        el.into_any_element()
    }));

    match app.store_tab {
        StoreTab::Catalog => {
            col = col.child(render_catalog(app, cx));
            col = col.child(super::store_apps::render_native_apps(app, cx, false));
        }
        // Native apps belong on both tabs — Installed shows just the live
        // ones, alongside the .kspkg package list.
        StoreTab::Installed => {
            col = col.child(super::store_apps::render_native_apps(app, cx, true));
            col = col.child(render_installed(app, cx));
        }
    }
    col.into_any_element()
}

fn render_catalog(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    slot_or(app, "store.catalog", |v| {
        let state = vstr(v, "state");
        let mut el = div().flex().flex_col().gap_3();
        if state == "unavailable" {
            el = el.child(card().child(row(
                "Каталог недоступен",
                "Маркетплейс не настроен для этой платформы или сборки.",
            )));
        }
        let listings = varr(v, "listings");
        if listings.is_empty() && state != "unavailable" {
            el = el.child(card().child(empty("Каталог пуст")));
        }
        for item in listings {
            let id = vstr(item, "id");
            let name = vopt(item, "name").unwrap_or_else(|| id.clone());
            let desc = vopt(item, "description").unwrap_or_default();
            let ver = vstr(item, "version");
            let kind = vstr(item, "kind");
            let mut r = row(
                name.clone(),
                if desc.is_empty() { kind.clone() } else { desc },
            );
            r = r.child(badge(format!("v{ver}"), MUTED_FG()));
            let install_id = id.clone();
            let detail_item = item.clone();
            r = r
                .child(
                    div()
                        .id(SharedString::from(format!("listing-{id}")))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(c(ACCENT()))
                        .child("Подробнее")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.detail = Some(detail_item.clone());
                            cx.notify();
                        }))
                        .role(Role::Button)
                        .aria_label(format!("Подробнее: {name}")),
                )
                .child(btn_id(&format!("install-{id}"), "Установить", {
                    let pid = install_id;
                    cx.listener(move |this, _, _, cx| {
                        this.pending_install = Some(pid.clone());
                        this.call(
                            "disclosure",
                            "packages.disclosure",
                            json!({"package_id": pid}),
                        );
                        cx.notify();
                    })
                }));
            el = el.child(card().child(r));
        }
        el.into_any_element()
    })
}

fn render_installed(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    slot_or(app, "store.installed", |v| {
        let items = varr(v, "packages");
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child(format!("Установленные пакеты ({})", items.len())),
        );
        if items.is_empty() {
            el = el.child(empty("Нет установленных пакетов"));
        }
        for p in items {
            let id = vstr(p, "id");
            let name = vopt(p, "name").unwrap_or_else(|| id.clone());
            let ver = vstr(p, "version");
            let enabled = vbool(p, "enabled");
            let status = if enabled {
                "Включён"
            } else {
                "Отключён"
            };
            let rid = id.clone();
            let uid = id.clone();
            el = el.child(
                row(format!("{name} · v{ver}"), id.clone())
                    .child(badge(status, if enabled { SUCCESS() } else { MUTED_FG() }))
                    .child(
                        toggle(
                            // leaks a key per package id — ids are stable and few
                            Box::leak(format!("en-{rid}").into_boxed_str()),
                            enabled,
                            cx,
                            move |this, checked, _| {
                                this.action(
                                    "packages.set_enabled",
                                    json!({"package_id": rid, "enabled": checked}),
                                );
                            },
                        )
                        .accessibility_label(format!("{name} · v{ver}")),
                    )
                    .child(btn_id(&format!("un-{id}"), "Удалить", {
                        let pid = uid;
                        cx.listener(move |this, _, _, cx| {
                            this.ask_confirm(
                                "Удалить пакет",
                                format!("Пакет «{name}» будет удалён из Engine."),
                                "packages.uninstall",
                                json!({"package_id": pid}),
                                cx,
                            );
                        })
                    })),
            );
        }
        el.into_any_element()
    })
}
