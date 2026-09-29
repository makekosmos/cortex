//! Native GPUI app rows for the Store view (KOS-265) — catalog-gated
//! Установить / Обновить / Открыть / Удалить.
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

/// Row actions for a native app entry — the state machine behind
/// Установить/Обновить/Открыть/Удалить. Kept pure so row states are unit-testable.
#[derive(Debug, PartialEq, Eq)]
enum NativeAction {
    Install,
    Update,
    Open,
    Uninstall,
}

fn native_actions(item: &serde_json::Value) -> Vec<NativeAction> {
    let update = item
        .get("update_version")
        .and_then(|v| v.as_str())
        .is_some_and(|v| !v.is_empty());
    match (vbool(item, "installed"), update) {
        (false, _) => vec![NativeAction::Install],
        (true, true) => vec![
            NativeAction::Update,
            NativeAction::Open,
            NativeAction::Uninstall,
        ],
        (true, false) => vec![NativeAction::Open, NativeAction::Uninstall],
    }
}

/// Native GPUI apps (KOS-265): catalog-gated rows with
/// Установить / Обновить / Открыть / Удалить.
pub(super) fn render_native_apps(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    slot_or(app, "store.apps", |v| {
        let items = varr(v, "apps");
        if items.is_empty() {
            return empty("").into_any_element();
        }
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child("Приложения"),
        );
        for item in items {
            let id = vstr(item, "id");
            let name = vopt(item, "name").unwrap_or_else(|| id.clone());
            let installed = vbool(item, "installed");
            let status = if vopt(item, "update_version").is_some_and(|v| !v.is_empty()) {
                format!(
                    "Обновление v{}",
                    vopt(item, "update_version").unwrap_or_default()
                )
            } else if installed {
                format!("Установлено v{}", vstr(item, "installed_version"))
            } else {
                format!("v{}", vstr(item, "catalog_version"))
            };
            let mut r = row(name.clone(), id.clone()).child(badge(
                status,
                if installed { SUCCESS() } else { MUTED_FG() },
            ));
            for action_kind in native_actions(item) {
                let aid = id.clone();
                let aname = name.clone();
                r = r.child(match action_kind {
                    NativeAction::Install => {
                        btn_id(&format!("app-install-{id}"), "Установить", {
                            cx.listener(move |this, _, _, cx| {
                                this.action("apps.install", json!({ "id": aid }));
                                cx.notify();
                            })
                        })
                    }
                    NativeAction::Update => {
                        btn_id(&format!("app-update-{id}"), "Обновить", {
                            cx.listener(move |this, _, _, cx| {
                                this.action("apps.install", json!({ "id": aid }));
                                cx.notify();
                            })
                        })
                    }
                    NativeAction::Open => btn_id(&format!("app-open-{id}"), "Открыть", {
                        cx.listener(move |this, _, _, cx| {
                            this.action("apps.launch", json!({ "id": aid }));
                            cx.notify();
                        })
                    }),
                    NativeAction::Uninstall => {
                        btn_id(&format!("app-uninstall-{id}"), "Удалить", {
                            cx.listener(move |this, _, _, cx| {
                                this.ask_confirm(
                                    "Удалить приложение",
                                    format!("Приложение «{aname}» будет удалено."),
                                    "apps.uninstall",
                                    json!({ "id": aid }),
                                    cx,
                                );
                            })
                        })
                    }
                });
            }
            el = el.child(r);
        }
        el.into_any_element()
    })
}

#[cfg(test)]
mod tests {
    use super::{native_actions, NativeAction};
    use serde_json::json;

    #[test]
    fn not_installed_native_app_offers_only_install() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "name": "Agenda",
            "installed": false,
            "installed_version": null,
            "catalog_version": "0.1.1",
            "update_available": false,
        });
        assert_eq!(native_actions(&item), vec![NativeAction::Install]);
    }

    #[test]
    fn installed_native_app_offers_open_and_uninstall() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "installed": true,
            "installed_version": "0.1.1",
            "catalog_version": "0.1.1",
            "update_available": false,
        });
        assert_eq!(
            native_actions(&item),
            vec![NativeAction::Open, NativeAction::Uninstall]
        );
    }

    #[test]
    fn installed_native_app_with_update_offers_update_open_uninstall() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "installed": true,
            "installed_version": "0.1.0",
            "catalog_version": "0.1.1",
            "update_version": "0.1.1",
        });
        assert_eq!(
            native_actions(&item),
            vec![
                NativeAction::Update,
                NativeAction::Open,
                NativeAction::Uninstall
            ]
        );
    }

    #[test]
    fn malformed_native_row_falls_back_to_install_only() {
        // A row missing `installed` must never offer Open/Uninstall — the
        // engine would reject the launch and removing a phantom record is a
        // no-op. Fail closed to Install.
        let item = json!({ "id": "com.kosmos.agenda" });
        assert_eq!(native_actions(&item), vec![NativeAction::Install]);
    }
}
