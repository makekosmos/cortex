//! О приложении: static structure renders immediately; only Engine field
//! values load asynchronously. Support tools are opt-in, never auto-fetched.
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::async_fields::{field_row, field_text};
use crate::theme::*;
use crate::widgets::*;

#[cfg(test)]
#[path = "about_tests.rs"]
mod tests;

pub fn load(app: &mut ManagerApp) {
    app.status("about.health", "health");
    app.status("about.info", "info");
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = page_stack();
    col = col.child(section("О приложении", "Версия и сведения"));

    let mut el = card()
        .id("about-product-card")
        .debug_selector(|| "about-product-card".into());
    // Shared build metadata is known immediately; bare Cargo is explicitly dev.
    el = el.child(kv("Mundus", crate::device_info::product_version_label()));
    col = col.child(el);

    let mut engine = card()
        .id("about-engine-card")
        .debug_selector(|| "about-engine-card".into())
        .child(
            div()
                .text_size(crate::theme::ui_px(12.))
                .text_color(c(MUTED_FG()))
                .child("Engine"),
        );
    for (key, label) in [
        ("version", "Версия"),
        ("api_version", "Версия API"),
        ("build", "Сборка"),
        ("channel", "Канал"),
    ] {
        engine = engine.child(
            field_row(app, "about.info", label, |v| {
                if key == "version" {
                    crate::device_info::version_label(&vstr(v, key), &vstr(v, "channel"))
                } else {
                    vstr(v, key)
                }
            })
            .id(format!("about-field-{key}"))
            .debug_selector(move || format!("about-field-{key}")),
        );
    }
    col = col.child(engine);

    let health = field_text(app.slots.get("about.health"), |v| {
        if vstr(v, "status") == "ready" {
            "Готов".into()
        } else {
            "Не готов".into()
        }
    });
    let healthy = health == "Готов";
    col = col.child(
        card().child(
            row("Состояние Engine", "Доступность локального сервиса")
                .child(badge(health, if healthy { SUCCESS() } else { MUTED_FG() })),
        ),
    );

    col = col.child(card().child(
        row("Поддержка", "Инструменты для разбора проблем с приложением").child(btn(
            "about-support-toggle",
            if app.about_support_open {
                "Скрыть"
            } else {
                "Показать"
            },
            false,
            cx,
            |app, cx| {
                app.about_support_open = !app.about_support_open;
                cx.notify();
            },
        )),
    ));
    if app.about_support_open {
        col = col.child(
            card()
                .child(
                    div()
                        .text_size(crate::theme::ui_px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Инструменты поддержки"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            crate::button::ghost("open-logs")
                                .label("Открыть папку журналов")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    match mundus_gpui_kit::engine::data_dir()
                                        .map(|d| d.join("logs"))
                                    {
                                        Ok(dir) => {
                                            std::fs::create_dir_all(&dir).ok();
                                            if let Err(e) = mundus_gpui_kit::engine::open_path(&dir)
                                            {
                                                this.error = Some(e);
                                            }
                                        }
                                        Err(e) => this.error = Some(e.message()),
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            crate::button::ghost("open-crashes")
                                .label("Открыть отчёты об ошибках")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    match mundus_gpui_kit::engine::data_dir()
                                        .map(|d| d.join("crashes"))
                                    {
                                        Ok(dir) => {
                                            std::fs::create_dir_all(&dir).ok();
                                            if let Err(e) = mundus_gpui_kit::engine::open_path(&dir)
                                            {
                                                this.error = Some(e);
                                            }
                                        }
                                        Err(e) => this.error = Some(e.message()),
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .text_size(crate::theme::ui_px(12.))
                        .text_color(c(MUTED_FG()))
                        .child("Снимок поддержки собирается Engine и сохраняется в файл."),
                )
                .child(
                    div().flex().gap_2().child(
                        crate::button::secondary("bundle")
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
            let name =
                vopt(&bundle, "suggested_name").unwrap_or_else(|| "mundus-support.zip".into());
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
                                crate::button::primary("bundle-save")
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
                                crate::button::ghost("bundle-cancel")
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
    }

    col.into_any_element()
}
