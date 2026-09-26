//! Настройки — автозапуск (Host-owned) + резервные копии БД
//! manager.db_backups.* (SettingsView.vue parity).
use ::gpui::{prelude::*, *};
use imago_gpui::button;
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("backups.list", "manager.db_backups.list", json!({}));
    app.call("engine.autostart", "engine.autostart.get", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Настройки", "Запуск Kosmos"));

    // Engine-owned autostart: registers `kepler-backend --start` in the HKCU
    // Run key — headless Engine at sign-in, no UI window (the standalone
    // Dictation app has its own Run entry and relies on Engine being up).
    col = col.child(slot_or(app, "engine.autostart", |v| {
        let available = vbool(v, "available");
        let enabled = vbool(v, "enabled");
        let mut row_el = row(
            "Автозапуск при входе",
            "Engine стартует с входом в Windows без UI (kepler-backend --start)",
        );
        if available {
            row_el = row_el.child(
                toggle("engine-autostart", enabled, cx, |this, on, cx| {
                    this.action("engine.autostart.set", json!({ "enabled": on }));
                    cx.notify();
                })
                .accessibility_label("Автозапуск при входе"),
            );
        } else {
            row_el = row_el.child(badge(
                &vopt(v, "reason").unwrap_or_else(|| "недоступно".into()),
                MUTED_FG(),
            ));
        }
        card().child(row_el).into_any_element()
    }));

    col = col.child(slot_or(app, "backups.list", |v| {
        let backups = varr(v, "backups");
        let mut el = card();
        el = el.child(
            row(
                "Резервные копии базы данных",
                format!(
                    "Последний снимок: {}",
                    backups
                        .first()
                        .map(|b| fmt_ms((js_now_ms() - vnum(b, "modified_ms")).max(0.0)))
                        .unwrap_or_else(|| "нет снимков".into())
                ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button::secondary("mk-backup")
                            .label("Сделать бэкап сейчас")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.action("manager.db_backups.create", json!({}));
                                cx.notify();
                            })),
                    )
                    .child(
                        button::ghost("open-backups")
                            .label("Открыть папку")
                            .on_click(cx.listener(|this, _, _, cx| {
                                match kosmos_gpui_kit::engine::data_dir().map(|d| d.join("backups"))
                                {
                                    Ok(dir) => {
                                        std::fs::create_dir_all(&dir).ok();
                                        if let Err(e) = kosmos_gpui_kit::engine::open_path(&dir) {
                                            this.error = Some(e);
                                        }
                                    }
                                    Err(e) => this.error = Some(e),
                                }
                                cx.notify();
                            })),
                    ),
            ),
        );
        for b in backups {
            let id = vstr(b, "id");
            let vid = id.clone();
            let rid = id.clone();
            el = el.child(
                row(
                    id.clone(),
                    format!(
                        "{} · {}",
                        fmt_bytes(vnum(b, "size_bytes")),
                        fmt_ms((js_now_ms() - vnum(b, "modified_ms")).max(0.0))
                    ),
                )
                .child(btn_id(&format!("val-{id}"), "Проверить", {
                    cx.listener(move |this, _, _, cx| {
                        this.call(
                            "backup.check",
                            "manager.db_backups.validate",
                            json!({"backup_id": vid}),
                        );
                        cx.notify();
                    })
                }))
                .child(btn_id(&format!("res-{id}"), "Восстановить", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Восстановить резервную копию",
                            "Текущая база будет заменена снимком. Engine перезапустит данные.",
                            "manager.db_backups.restore",
                            json!({"backup_id": rid}),
                            cx,
                        );
                    })
                })),
            );
        }
        el.into_any_element()
    }));

    // Validation result of the last check (if any).
    let check = app.data("backup.check");
    if !check.is_null() {
        let ok = vbool(&check, "ok");
        let reason = vopt(&check, "reason").unwrap_or_default();
        col = col.child(
            card().child(
                row(
                    "Результат проверки",
                    if reason.is_empty() {
                        if ok {
                            "Снимок целостен.".into()
                        } else {
                            "Снимок повреждён.".into()
                        }
                    } else {
                        reason
                    },
                )
                .child(badge(
                    if ok { "OK" } else { "Ошибка" },
                    if ok { SUCCESS() } else { DESTRUCTIVE() },
                )),
            ),
        );
    }

    col.into_any_element()
}

fn js_now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}
