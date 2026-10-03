//! Database backup controls belong to Data; operation names and confirmations stay unchanged.
use crate::app::ManagerApp;
use crate::async_fields::field_text;
use crate::widgets::*;
use ::gpui::{prelude::*, *};
use mundus_gpui_kit::theme::*;
use serde_json::json;

pub fn load(app: &mut ManagerApp) {
    app.call("backups.list", "manager.db_backups.list", json!({}));
}

pub fn render(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let v = app.data("backups.list");
    let ready = matches!(
        app.slots.get("backups.list"),
        Some(crate::app::Slot::Ready(_))
    );
    let detail = field_text(app.slots.get("backups.list"), |v| {
        let backups = varr(v, "backups");
        format!(
            "Последний снимок: {}",
            backups
                .first()
                .map(|b| fmt_ms((now_ms() - vnum(b, "modified_ms")).max(0.)))
                .unwrap_or_else(|| "нет снимков".into())
        )
    });
    let mut content = card()
        .id("data-backups-card")
        .debug_selector(|| "data-backups-card".into())
        .child(row("Резервные копии базы данных", detail))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    crate::button::secondary("mk-backup")
                        .label("Создать копию")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.action("manager.db_backups.create", json!({}));
                            cx.notify();
                        })),
                )
                .child(
                    crate::button::ghost("open-backups")
                        .label("Открыть папку")
                        .on_click(cx.listener(|this, _, _, cx| {
                            match mundus_gpui_kit::engine::data_dir().map(|d| d.join("backups")) {
                                Ok(dir) => {
                                    std::fs::create_dir_all(&dir).ok();
                                    if let Err(e) = mundus_gpui_kit::engine::open_path(&dir) {
                                        this.error = Some(e);
                                    }
                                }
                                Err(e) => this.error = Some(e.message()),
                            }
                            cx.notify();
                        })),
                ),
        );
    if ready && varr(&v, "backups").is_empty() {
        content = content.child(empty("Сохранённых копий пока нет"));
    }
    for backup in varr(&v, "backups") {
        let id = vstr(backup, "id");
        let validate = id.clone();
        let restore = id.clone();
        content = content.child(
            row(
                id.clone(),
                format!(
                    "{} · {}",
                    fmt_bytes(vnum(backup, "size_bytes")),
                    fmt_ms((now_ms() - vnum(backup, "modified_ms")).max(0.))
                ),
            )
            .child(btn_id(
                &format!("val-{id}"),
                "Проверить",
                cx.listener(move |this, _, _, cx| {
                    this.call(
                        "backup.check",
                        "manager.db_backups.validate",
                        json!({"backup_id":validate}),
                    );
                    cx.notify();
                }),
            ))
            .child(btn_id(
                &format!("res-{id}"),
                "Восстановить",
                cx.listener(move |this, _, _, cx| {
                    this.ask_confirm(
                        "Восстановить резервную копию",
                        "Текущая база будет заменена снимком. Engine перезапустит данные.",
                        "manager.db_backups.restore",
                        json!({"backup_id":restore}),
                        cx,
                    );
                }),
            )),
        );
    }
    let check = app.data("backup.check");
    if !check.is_null() {
        let ok = vbool(&check, "ok");
        let reason = vopt(&check, "reason")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                if ok {
                    "Снимок целостен.".into()
                } else {
                    "Снимок повреждён.".into()
                }
            });
        content = content.child(row("Результат проверки", reason).child(badge(
            if ok { "OK" } else { "Ошибка" },
            if ok { SUCCESS() } else { DESTRUCTIVE() },
        )));
    }
    content.into_any_element()
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_millis() as f64)
        .unwrap_or(0.)
}
