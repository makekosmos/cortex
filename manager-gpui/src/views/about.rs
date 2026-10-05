//! О приложении: static structure renders immediately; only Engine field
//! values load asynchronously. Support tools are opt-in, never auto-fetched.
use ::gpui::{prelude::*, AnimationExt, *};
use serde_json::json;
use std::time::Duration;

use crate::app::{ManagerApp, Slot};

use crate::theme::*;
use crate::widgets::*;

#[cfg(test)]
#[path = "about_tests.rs"]
mod tests;

pub fn load(app: &mut ManagerApp) {
    super::updates::load(app);
    app.status("about.health", "health");
    app.call("about.catalog", "packages.catalog_status", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = page_stack();

    // Raycast-style hero: the product mark (same source as the tray icon),
    // name, version and copyright — known immediately, never async.
    col = col.child(
        div()
            .id("about-hero")
            .debug_selector(|| "about-hero".into())
            .flex()
            .flex_col()
            .items_center()
            .pt_8()
            .pb_6()
            .child(
                gpui_component::Icon::default()
                    .path("icons/mundus.svg")
                    .size(px(96.))
                    .text_color(c(ACCENT())),
            )
            .child(
                div()
                    .pt_3()
                    .text_size(crate::theme::ui_px(22.))
                    .font_weight(FontWeight::BOLD)
                    .child("Mundus"),
            )
            .child(
                div()
                    .pt_1()
                    .text_size(crate::theme::ui_px(13.))
                    .text_color(c(MUTED_FG()))
                    // A bare `cargo build` has no injected product version and
                    // reports "dev" here — never the crate's 0.1.0 placeholder
                    // (KOS-278).
                    .child(crate::device_info::product_version_label()),
            )
            .child(
                div()
                    .pt_2()
                    .text_size(crate::theme::ui_px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("© Yoso Industries 2025–2026"),
            ),
    );

    col = col.child(render_diagnostics(app, cx));
    col = col.child(super::updates::render(app, cx));

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

/// Диагностика — Raycast-style: «Все проверки пройдены» пока каждая проверка
/// зелёная; при сбоях заголовок раскрывает список проблем по кнопке.
enum Diag {
    Ok,
    Pending,
    Fail,
}

type Check = (&'static str, Diag);

fn render_diagnostics(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let checks: Vec<Check> = vec![
        (
            "Engine",
            match app.slots.get("about.health") {
                Some(Slot::Ready(v)) if vstr(v, "status") == "ready" => Diag::Ok,
                Some(Slot::Ready(_)) | Some(Slot::Failed(_)) => Diag::Fail,
                _ => Diag::Pending,
            },
        ),
        (
            "Каталог интеграций",
            match app.slots.get("about.catalog") {
                Some(Slot::Ready(v))
                    if vget(v, "catalog").is_object()
                        && vopt(v, "fault").unwrap_or_default().is_empty() =>
                {
                    Diag::Ok
                }
                Some(Slot::Ready(_)) => Diag::Fail,
                Some(Slot::Failed(_)) => Diag::Fail,
                _ => Diag::Pending,
            },
        ),
    ];
    let (bad, good): (Vec<Check>, Vec<Check>) = checks
        .into_iter()
        .partition(|(_, d)| matches!(d, Diag::Fail));
    let has_failures = !bad.is_empty();
    let pending = good.iter().any(|(_, d)| matches!(d, Diag::Pending));

    let status_icon = |path: &'static str, color: u32| {
        gpui_component::Icon::default()
            .path(path)
            .size(px(18.))
            .text_color(c(color))
    };

    let mut body =
        card()
            .id("about-diagnostics-card")
            .debug_selector(|| "about-diagnostics-card".into())
            .px(px(0.))
            .py(px(0.))
            .gap(px(0.))
            .child(
                div()
                    .id("about-diagnostics-toggle")
                    .w_full()
                    // Список под шапкой закрывает нижние углы — скруглять
                    // низ имеет смысл только когда карточка свёрнута.
                    .rounded_t(px(12.))
                    .when(!app.about_diag_open, |d| d.rounded_b(px(12.)))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px(px(crate::page_layout::INSET))
                    .py(px(10.))
                    .child(status_icon(
                        if has_failures {
                            "icons/circle-x.svg"
                        } else {
                            "icons/circle-check.svg"
                        },
                        if has_failures {
                            DESTRUCTIVE()
                        } else {
                            SUCCESS()
                        },
                    ))
                    .child(div().flex_1().text_size(crate::theme::ui_px(13.)).child(
                        if has_failures {
                            "Есть ошибки"
                        } else if pending {
                            "Проверка…"
                        } else {
                            "Всё в порядке"
                        },
                    ))
                    .when(has_failures, |d| {
                        d.child(
                            gpui_component::Icon::default()
                                .path(if app.about_diag_open {
                                    "icons/chevron-up.svg"
                                } else {
                                    "icons/chevron-down.svg"
                                })
                                .size(px(16.))
                                .text_color(c(MUTED_FG())),
                        )
                    })
                    .when(has_failures, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(fade(FG(), 0.05)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.about_diag_open = !this.about_diag_open;
                                cx.notify();
                            }))
                            .role(Role::Button)
                            .aria_label("Список проверок диагностики")
                    }),
            );

    if has_failures && app.about_diag_open {
        // Ошибки сверху, прошедшие проверки ниже.
        let mut list = div()
            .id("about-diagnostics-list")
            .debug_selector(|| "about-diagnostics-list".into())
            .w_full()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(fade(BORDER(), 0.6));
        for (i, (label, state)) in bad.iter().chain(good.iter()).enumerate() {
            let (path, color) = match state {
                Diag::Ok => ("icons/circle-check.svg", SUCCESS()),
                Diag::Pending => ("icons/circle-check.svg", MUTED_FG()),
                Diag::Fail => ("icons/circle-x.svg", DESTRUCTIVE()),
            };
            let mut r = div()
                .w_full()
                .flex()
                .items_center()
                .gap_3()
                .px(px(crate::page_layout::INSET))
                .py(px(8.))
                .child(status_icon(path, color))
                .child(div().text_size(crate::theme::ui_px(13.)).child(*label));
            if i > 0 {
                r = r.border_t_1().border_color(fade(BORDER(), 0.6));
            }
            list = list.child(r);
        }
        let rows = (bad.len() + good.len()) as f32;
        // 8px padding ×2 + ~18px icon per row, plus the top hairline.
        let full_h = rows * 34. + 1.;
        body = body.child(
            div()
                .w_full()
                .overflow_hidden()
                .with_animation(
                    "about-diagnostics-reveal",
                    Animation::new(Duration::from_millis(220))
                        .with_easing(|t| 1. - (1. - t).powi(3)),
                    move |element, progress| {
                        element
                            .h(px(full_h * progress))
                            .opacity(0.4 + 0.6 * progress)
                    },
                )
                .child(list),
        );
    }

    section_group()
        .child(
            div()
                .text_size(crate::theme::ui_px(13.))
                .child("Диагностика"),
        )
        .child(body)
        .into_any_element()
}
