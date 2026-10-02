//! Native GPUI app rows for the Store view (KOS-265) — the Engine hardcodes
//! the app list and checks each app's GitHub Releases for updates.
//! Installs run as Engine-side background jobs: this view polls `apps.list`
//! (see `ManagerApp::drain`) and renders the per-row state machine.
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

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
    let installed = vbool(item, "installed");
    let update = vopt(item, "update_version").is_some_and(|v| !v.is_empty());
    match vstr(item, "state").as_str() {
        // An in-flight job owns the row — a second tap would only bounce off
        // the Engine's per-app busy lock.
        "installing" => vec![],
        _ if installed && update => {
            vec![
                NativeAction::Update,
                NativeAction::Open,
                NativeAction::Uninstall,
            ]
        }
        _ if installed => vec![NativeAction::Open, NativeAction::Uninstall],
        // Offline/unsupported rows offer no network actions.
        "offline" | "unsupported" => vec![],
        // not-installed and failed rows both recover via Install.
        _ => vec![NativeAction::Install],
    }
}

/// Badge text + colour for a row's `state`/`failure` fields.
fn native_status(item: &serde_json::Value) -> (String, u32) {
    let installed = vbool(item, "installed");
    match vstr(item, "state").as_str() {
        "installing" => {
            let (done, total) = (vnum(item, "download_bytes"), vnum(item, "download_total"));
            if total > 0.0 {
                (format!("Установка… {:.0}%", done * 100.0 / total), ACCENT())
            } else {
                ("Установка…".into(), ACCENT())
            }
        }
        "failed" => {
            let detail = match vstr(item, "failure").as_str() {
                "busy" => "установка уже идёт".to_string(),
                "app-running" => "закройте приложение и повторите".to_string(),
                "offline" => "нет сети".to_string(),
                "integrity" => "архив не прошёл проверку".to_string(),
                "io" => "ошибка записи на диск".to_string(),
                "unsupported" => "не поддерживается".to_string(),
                other => other.to_string(),
            };
            (format!("Ошибка: {detail}"), DESTRUCTIVE())
        }
        _ if vopt(item, "update_version").is_some_and(|v| !v.is_empty()) => (
            format!("Обновление v{}", vstr(item, "update_version")),
            SUCCESS(),
        ),
        _ if installed => (
            format!("Установлено v{}", vstr(item, "installed_version")),
            SUCCESS(),
        ),
        "offline" => ("Нет сети".into(), MUTED_FG()),
        "unsupported" => ("Недоступно".into(), MUTED_FG()),
        _ => (format!("v{}", vstr(item, "latest_version")), MUTED_FG()),
    }
}

/// Native GPUI apps (KOS-265): GitHub Releases rows with
/// Установить / Обновить / Открыть / Удалить. `installed_only` narrows the
/// list for the Установленные tab. `extra` appends already-built rows
/// (app-kind .kspkg packages, KOS-283) inside the same «Приложения» card.
pub(super) fn render_native_apps(
    app: &mut ManagerApp,
    cx: &mut Context<ManagerApp>,
    installed_only: bool,
    extra: Vec<Div>,
) -> AnyElement {
    slot_or(app, "store.apps", |v| {
        let items: Vec<&serde_json::Value> = varr(v, "apps")
            .iter()
            .filter(|item| !installed_only || vbool(item, "installed"))
            .collect();
        if items.is_empty() && extra.is_empty() {
            // Nothing to show — render nothing rather than a blank padded
            // card.
            return div().into_any_element();
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
            // The app id is an internal key — never a headline or caption
            // (KOS-279). The badge already carries the version/status.
            let name = vopt(item, "name").unwrap_or_else(|| "Приложение".into());
            let (status, color) = native_status(item);
            let mut r = entry_row(
                icon_file(vopt(item, "icon_path")),
                name.clone(),
                String::new(),
            )
            .child(badge(status, color));
            for action_kind in native_actions(item) {
                let aid = id.clone();
                let aname = name.clone();
                r = r.child(match action_kind {
                    // Install and update are the same Engine op — the reply
                    // goes to a side slot (`apps.op`) and drain re-pulls the
                    // list, so starting a job never blanks the rows.
                    NativeAction::Install | NativeAction::Update => {
                        let (element_id, label) = if action_kind == NativeAction::Install {
                            (format!("app-install-{id}"), "Установить")
                        } else {
                            (format!("app-update-{id}"), "Обновить")
                        };
                        btn_id(&element_id, label, {
                            cx.listener(move |this, _, _, cx| {
                                this.refresh("apps.op", "apps.install", json!({ "id": aid }));
                                cx.notify();
                            })
                        })
                    }
                    NativeAction::Open => btn_id(&format!("app-open-{id}"), "Открыть", {
                        cx.listener(move |this, _, _, cx| {
                            this.action("apps.open", json!({ "id": aid }));
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
        for row in extra {
            el = el.child(row);
        }
        el.into_any_element()
    })
}

#[cfg(test)]
mod tests {
    use super::{native_actions, native_status, NativeAction};
    use serde_json::json;

    #[test]
    fn not_installed_native_app_offers_only_install() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "name": "Agenda",
            "installed": false,
            "installed_version": null,
            "latest_version": "0.1.1",
            "state": "not-installed",
        });
        assert_eq!(native_actions(&item), vec![NativeAction::Install]);
    }

    #[test]
    fn installed_native_app_offers_open_and_uninstall() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "installed": true,
            "installed_version": "0.1.1",
            "latest_version": "0.1.1",
            "state": "installed",
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
            "latest_version": "0.1.1",
            "update_version": "0.1.1",
            "state": "update-available",
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
    fn installing_row_offers_no_actions_and_reports_progress() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "state": "installing",
            "download_bytes": 50,
            "download_total": 100,
        });
        assert_eq!(native_actions(&item), Vec::<NativeAction>::new());
        let (label, _) = native_status(&item);
        assert_eq!(label, "Установка… 50%");
    }

    #[test]
    fn failed_row_retries_install_and_shows_the_typed_reason() {
        let item = json!({
            "id": "com.kosmos.agenda",
            "installed": false,
            "state": "failed",
            "failure": "app-running",
        });
        assert_eq!(native_actions(&item), vec![NativeAction::Install]);
        let (label, _) = native_status(&item);
        assert_eq!(label, "Ошибка: закройте приложение и повторите");
    }

    #[test]
    fn failed_installed_row_keeps_open_and_uninstall() {
        // A failed UPDATE leaves the old version live — its row must still
        // offer Open/Uninstall.
        let item = json!({
            "id": "com.kosmos.agenda",
            "installed": true,
            "installed_version": "0.1.0",
            "state": "failed",
            "failure": "integrity",
        });
        assert_eq!(
            native_actions(&item),
            vec![NativeAction::Open, NativeAction::Uninstall]
        );
    }

    #[test]
    fn offline_rows_offer_no_network_action_but_installed_still_opens() {
        let absent = json!({ "id": "x", "installed": false, "state": "offline" });
        assert_eq!(native_actions(&absent), Vec::<NativeAction>::new());
        let present = json!({
            "id": "x", "installed": true, "installed_version": "1.0", "state": "offline",
        });
        assert_eq!(
            native_actions(&present),
            vec![NativeAction::Open, NativeAction::Uninstall]
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
