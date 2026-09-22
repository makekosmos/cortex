//! О приложении — /v1/health, /v1/info, diagnostics snapshot + log tail +
//! support bundle (AboutView.vue parity). Version is the crate version plus
//! whatever the Engine reports.
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.status("about.health", "health");
    app.status("about.info", "info");
    app.call("about.diag", "manager.diagnostics.snapshot", json!({}));
    app.call(
        "about.logs",
        "manager.diagnostics.log_tail",
        json!({"lines": 50}),
    );
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("О приложении", "Версия и сведения о Kosmos"));

    let mut el = card();
    el = el.child(kv("Manager (GPUI)", env!("CARGO_PKG_VERSION")));
    col = col.child(el);

    col = col.child(slot_or(app, "about.info", |v| {
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child("Engine"),
        );
        for key in ["version", "api_version", "build", "channel"] {
            let val = vopt(v, key);
            if let Some(val) = val {
                el = el.child(kv(key, val));
            }
        }
        if v.is_null() {
            el = el.child(empty("Engine не отвечает"));
        }
        el.into_any_element()
    }));

    col = col.child(slot_or(app, "about.health", |v| {
        let ok = vbool(v, "ok") || vstr(v, "status") == "ready";
        card()
            .child(row("Здоровье Engine", "Проверка /v1/health").child(badge(
                if ok { "Готов" } else { "Не готов" },
                if ok { SUCCESS() } else { DESTRUCTIVE() },
            )))
            .into_any_element()
    }));

    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Диагностика"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        imago_gpui::button::ghost("open-logs")
                            .label("Открыть папку журналов")
                            .on_click(cx.listener(|this, _, _, cx| {
                                match crate::engine::data_dir().map(|d| d.join("logs")) {
                                    Ok(dir) => {
                                        std::fs::create_dir_all(&dir).ok();
                                        if let Err(e) = crate::engine::open_path(&dir) {
                                            this.error = Some(e);
                                        }
                                    }
                                    Err(e) => this.error = Some(e),
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        imago_gpui::button::ghost("open-crashes")
                            .label("Открыть отчёты об ошибках")
                            .on_click(cx.listener(|this, _, _, cx| {
                                match crate::engine::data_dir().map(|d| d.join("crashes")) {
                                    Ok(dir) => {
                                        std::fs::create_dir_all(&dir).ok();
                                        if let Err(e) = crate::engine::open_path(&dir) {
                                            this.error = Some(e);
                                        }
                                    }
                                    Err(e) => this.error = Some(e),
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Снимок поддержки собирается Engine и сохраняется в файл."),
            )
            .child(
                div().flex().gap_2().child(
                    imago_gpui::button::secondary("bundle")
                        .label("Создать пакет поддержки")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.call(
                                "@bundle",
                                "manager.diagnostics.support_bundle.create",
                                json!({}),
                            );
                            cx.notify();
                        })),
                ),
            ),
    );

    let bundle = app.data("@bundle");
    if !bundle.is_null() {
        let handle = vstr(&bundle, "handle");
        let name = vopt(&bundle, "suggested_name").unwrap_or_else(|| "kosmos-support.zip".into());
        let path = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("Downloads")
            .join(&name);
        let path_str = path.to_string_lossy().to_string();
        col = col.child(
            card()
                .child(row("Пакет создан", format!("Сохранить как {name}?")))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            imago_gpui::button::primary("bundle-save")
                                .label(format!("Сохранить в {path_str}"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.action(
                                        "manager.diagnostics.support_bundle.save",
                                        json!({"handle": handle, "destination": path_str}),
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            imago_gpui::button::ghost("bundle-cancel")
                                .label("Отмена")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let h = vstr(&this.data("@bundle"), "handle");
                                    if !h.is_empty() {
                                        this.action(
                                            "manager.diagnostics.support_bundle.cancel",
                                            json!({"handle": h}),
                                        );
                                    }
                                    this.slots.remove("@bundle");
                                    cx.notify();
                                })),
                        ),
                ),
        );
    }

    col = col.child(slot_or(app, "about.logs", |v| {
        let entries = varr(v, "entries");
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child(format!("Журнал диагностики ({} записей)", entries.len())),
        );
        if entries.is_empty() {
            el = el.child(empty("Записей пока нет"));
        }
        for e in entries.iter().take(50) {
            let op = vstr(e, "operation");
            let stats = vget(e, "stats");
            el = el.child(kv(
                &op,
                format!(
                    "вызовов {} · p95 {} мс",
                    vstr(stats, "count"),
                    vstr(stats, "p95_ms")
                ),
            ));
        }
        el.into_any_element()
    }));

    col.into_any_element()
}
