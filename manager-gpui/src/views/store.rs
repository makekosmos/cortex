//! Маркетплейс — store.catalog/refresh/external_url + packages.list/install/
//! set_enabled/uninstall/catalog_status/disclosure (StoreView+PackagesView parity).
//!
//! KOS-283/285: installed packages split by the manifest `kind` the Engine
//! already serves (`PackageSummary.kind`): `app` rows join the «Приложения»
//! card however unfinished they are, `source`/`bridge` stay in «Пакеты».
//! Every row shows the product icon (manifest `icon` → `icon_path`,
//! catalog `icon_url`) and the display name; the package id is the caption.
use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::theme::*;
use crate::views::StoreTab;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("store.catalog", "store.catalog", json!({}));
    app.call("store.installed", "packages.list", json!({}));
    app.call("store.status", "packages.catalog_status", json!({}));
    // KOS-265 native GPUI apps — separate install path from .kspkg packages.
    app.call("store.apps", "apps.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let _ = window;
    let mut col = page_stack();
    col = col.child(section("Приложения", "Установленные приложения и каталог"));

    // Tab row: Каталог / Установленные
    let mut tabs = div().flex().flex_wrap().items_center().gap_2();
    for (tab, label) in [
        (StoreTab::Installed, "Установленные"),
        (StoreTab::Catalog, "Каталог"),
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
                .text_size(crate::theme::ui_px(13.))
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

    match app.store_tab {
        StoreTab::Catalog => {
            col = col.child(render_catalog(app, cx));
            if let Some(native) = super::store_apps::render_native_apps(app, cx, false, vec![]) {
                col = col.child(native);
            }
            col = col.child(slot_or(app, "store.status", |v| {
                let catalog = vget(v, "catalog");
                let fault = vopt(v, "fault");
                let loaded = catalog.is_object();
                let mut el = card();
                el = el.child(
                    row(
                        "Каталог интеграций",
                        "Загружается с GitHub, пакеты проверяются по контрольным суммам",
                    )
                    .child(badge(
                        if loaded {
                            "Загружен"
                        } else {
                            "Не загружен"
                        },
                        if loaded { SUCCESS() } else { WARN() },
                    )),
                );
                el = el.child(kv("Версия каталога", vstr(catalog, "sequence")));
                el = el.child(kv("Отозванные пакеты", vstr(catalog, "revoked_count")));
                if let Some(fault) = fault {
                    if !fault.is_empty() {
                        el = el.child(kv("Состояние", fault));
                    }
                }
                el.into_any_element()
            }));
        }
        // Native apps belong on both tabs — Installed shows just the live
        // ones, alongside the app-kind .kspkg packages in the same card.
        StoreTab::Installed => {
            let package_apps = app_package_rows(app, cx);
            if let Some(native) = super::store_apps::render_native_apps(app, cx, true, package_apps)
            {
                col = col.child(native);
            }
            col = col.child(render_installed(app, cx));
        }
    }
    col.into_any_element()
}

/// An installed package is an app exactly when its verified manifest says so
/// — `kind` on `packages.list` is serialized `PackageKind`, not a guess.
fn is_app_package(p: &Value) -> bool {
    vstr(p, "kind") == "app"
}

/// Dev-installed records have no signed catalog entry (`catalog_sequence`
/// 0) — they are the unfinished apps the «В разработке» badge is for.
pub(crate) fn is_development(p: &Value) -> bool {
    // Missing field ≠ 0: the field always serializes, so its absence means
    // the row is malformed — not a dev install.
    p.get("catalog_sequence").and_then(Value::as_u64) == Some(0)
}

/// Display name only — the package id (`com.kosmos.*`) is an internal key
/// and never reaches the screen, not even as a fallback (KOS-279).
fn entry_title(p: &Value) -> String {
    vopt(p, "name")
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Без названия".into())
}

/// Package `kind` → Russian UI text for catalog captions.
fn kind_text(kind: &str) -> &'static str {
    match kind {
        "app" => "Приложение",
        "source" => "Источник данных",
        "bridge" => "Мост",
        "widget" => "Виджет",
        _ => "Пакет",
    }
}

/// App-kind rows from `packages.list` for the «Приложения» card. An app
/// stays an app however unfinished it is — the readiness badge says
/// «В разработке» instead of moving the row to «Пакеты».
fn app_package_rows(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> Vec<Div> {
    varr(&app.data("store.installed"), "packages")
        .iter()
        .filter(|p| is_app_package(p))
        .map(|p| package_row(p, cx))
        .collect()
}

/// «Открыть» disabled flag — a disabled package can never launch, so the
/// control is disabled rather than letting the click fail. The vendored
/// a11y tree does not surface `disabled`, so tests assert this predicate.
pub(crate) fn package_open_disabled(p: &Value) -> bool {
    !vbool(p, "enabled")
}

/// One installed package row: icon, display name, `v<version>` caption,
/// status badge, enable toggle and Удалить.
fn package_row(p: &Value, cx: &mut Context<ManagerApp>) -> Div {
    let id = vstr(p, "id");
    let name = entry_title(p);
    let enabled = vbool(p, "enabled");
    let status = if enabled {
        "Включён"
    } else {
        "Отключён"
    };
    let rid = id.clone();
    let uid = id.clone();
    let mut r = entry_row(
        icon_file(vopt(p, "icon_path")),
        name.clone(),
        format!("v{}", vstr(p, "version")),
    );
    if is_development(p) {
        r = r.child(badge("В разработке", WARN()));
    }
    r = r
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
            .accessibility_label(name.clone()),
        );
    // App-kind rows get «Открыть» like native apps (KOS-299): packages.open
    // mints the launch lease and the reply opens it in the system browser.
    if is_app_package(p) {
        let oid = id.clone();
        let oversion = vstr(p, "version");
        r = r.child(
            btn_id(
                &format!("open-{id}"),
                "Открыть",
                cx.listener(move |this, _, _, cx| {
                    this.open_package(oid.clone(), oversion.clone());
                    cx.notify();
                }),
            )
            .disabled(package_open_disabled(p))
            .accessibility_label(format!("Открыть {name}")),
        );
    }
    r = r.child(btn_id(&format!("un-{id}"), "Удалить", {
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
    }));
    r
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
            let name = entry_title(item);
            let desc = vopt(item, "description").unwrap_or_default();
            let ver = vstr(vget(item, "distribution"), "version");
            let kind = vstr(item, "kind");
            // The caption carries meaning: the description, else the kind
            // translated to Russian. The package id never shows (KOS-279).
            let caption = if desc.is_empty() {
                kind_text(&kind).to_string()
            } else {
                desc
            };
            let mut r = entry_row(icon_url(vopt(item, "icon_url")), name.clone(), caption);
            r = r.child(badge(format!("v{ver}"), MUTED_FG()));
            let install_id = id.clone();
            let detail_item = item.clone();
            r = r
                .child(
                    div()
                        .id(SharedString::from(format!("listing-{id}")))
                        .cursor_pointer()
                        .text_size(crate::theme::ui_px(13.))
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
        // App-kind entries live in the «Приложения» card — this card lists
        // only integrations and other non-app packages.
        let items: Vec<&Value> = varr(v, "packages")
            .iter()
            .filter(|p| !is_app_package(p))
            .collect();
        let mut el = card();
        el = el.child(
            div()
                .text_size(crate::theme::ui_px(12.))
                .text_color(c(MUTED_FG()))
                .child(format!("Установленные пакеты ({})", items.len())),
        );
        if items.is_empty() {
            el = el.child(empty("Нет установленных пакетов"));
        }
        for p in items {
            el = el.child(package_row(p, cx));
        }
        el.into_any_element()
    })
}

#[cfg(test)]
mod tests {
    use super::{entry_title, is_app_package, is_development};
    use serde_json::json;

    #[test]
    fn app_kind_lands_in_apps_whatever_its_readiness() {
        // KOS-283: an unfinished (dev-installed, catalog_sequence 0) app is
        // still an app — it must not fall back to «Установленные пакеты».
        for p in [
            json!({"id": "com.kosmos.arcadia", "kind": "app", "catalog_sequence": 15}),
            json!({"id": "com.kosmos.arcadia", "kind": "app", "catalog_sequence": 0}),
            json!({"id": "com.kosmos.focus", "kind": "app"}),
        ] {
            assert!(is_app_package(&p), "{p}");
        }
        for p in [
            json!({"id": "com.kosmos.codewars", "kind": "source"}),
            json!({"id": "com.kosmos.bridge", "kind": "bridge"}),
            json!({"id": "com.kosmos.unknown", "kind": "widget"}),
            json!({"id": "com.kosmos.broken"}),
        ] {
            assert!(!is_app_package(&p), "{p}");
        }
    }

    #[test]
    fn development_marker_is_the_absent_catalog_entry() {
        // install_development lands with catalog_sequence 0 — that, not the
        // id or version, is what marks a row «В разработке».
        assert!(is_development(&json!({"catalog_sequence": 0})));
        assert!(!is_development(&json!({"catalog_sequence": 15})));
        assert!(!is_development(&json!({})));
    }

    #[test]
    fn display_name_wins_and_the_id_never_leaks() {
        // KOS-285: «Ordo» is the headline. KOS-279: a missing name degrades
        // to a neutral placeholder, never to the internal package id.
        let p = json!({"id": "com.kosmos.focus", "name": "Ordo"});
        assert_eq!(entry_title(&p), "Ordo");
        assert_eq!(
            entry_title(&json!({"id": "com.kosmos.focus"})),
            "Без названия"
        );
        assert_eq!(
            entry_title(&json!({"id": "com.kosmos.focus", "name": ""})),
            "Без названия"
        );
    }
}
