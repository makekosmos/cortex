//! Разработка — dev environment info + packages.install_development
//! (DevPanelView.vue parity). Dev package discovery stays Host-owned
//! (dev-packages.json is repo-local); GPUI installs by explicit path.
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("dev.packages", "packages.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Разработка", "Локальные пакеты и инстанс"));

    let mut mode = div().flex().gap_2();
    for (index, label) in ["Светлая", "Тёмная", "Системная"].iter().enumerate()
    {
        let active = app.theme_mode == index as u8;
        mode = mode.child(
            imago_gpui::button::button(
                SharedString::from(format!("theme-mode-{index}")),
                if active {
                    imago_gpui::button::ButtonKind::Primary
                } else {
                    imago_gpui::button::ButtonKind::Ghost
                },
            )
            .label(*label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.theme_mode = index as u8;
                cx.notify();
            })),
        );
    }
    let system_dark = matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    let dark = match app.theme_mode {
        0 => false,
        2 => system_dark,
        _ => true,
    };
    let mut themes = div().flex().flex_col().gap_2();
    for (index, definition) in imago_gpui::THEMES.iter().enumerate() {
        let active = app.theme_idx == index;
        let palette = if dark {
            &definition.dark
        } else {
            &definition.light
        };
        let mut swatches = div().flex().items_center().gap_1();
        for color in [palette.accent, palette.card, palette.fg] {
            swatches = swatches.child(
                div()
                    .size_3()
                    .rounded_full()
                    .border_1()
                    .border_color(c(BORDER()))
                    .bg(c(color)),
            );
        }
        themes = themes.child(
            div()
                .id(SharedString::from(format!("theme-{index}")))
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(if active {
                    fade(ACCENT(), 0.5)
                } else {
                    fade(FG(), 0.10)
                })
                .bg(if active {
                    fade(ACCENT(), 0.08)
                } else {
                    fade(FG(), 0.02)
                })
                .cursor_pointer()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(div().text_size(px(13.)).child(definition.name))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(c(MUTED_FG()))
                                .child(definition.desc),
                        ),
                )
                .child(swatches)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.theme_idx = index;
                    cx.notify();
                }))
                .role(Role::Button)
                .aria_label(format!("Тема: {}", definition.name))
                .aria_selected(active),
        );
    }
    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Интерфейс"),
            )
            .child(row("Режим", "Светлая, тёмная или системная тема").child(mode))
            .child(themes)
            .child(
                row(
                    "FPS-счётчик",
                    "График, средний FPS, 1% и 0.1% low в правом нижнем углу.",
                )
                .child(
                    toggle("dev-fps", app.dev_fps, cx, |this, checked, _| {
                        this.dev_fps = checked;
                    })
                    .accessibility_label("FPS-счётчик"),
                ),
            ),
    );

    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Инстанс"),
            )
            .child(kv("Режим", "GPUI Manager"))
            .child(kv(
                "Данные",
                mundus_gpui_kit::engine::data_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "не найдена".into()),
            ))
            .child(kv(
                "Dev-пакеты",
                "discovery через dev-packages.json — Host; установка по пути доступна",
            )),
    );

    let path_in = app.input(
        "dev.path",
        "Путь к release/*.kspkg или папке пакета…",
        window,
        cx,
    );
    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Установить dev-пакет"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&path_in)))
                    .child(btn(
                        "dev-install",
                        "Установить",
                        true,
                        cx,
                        |this, cx| {
                            let p = this.input_value("dev.path", cx);
                            if p.is_empty() {
                                return;
                            }
                            match crate::devpkg::resolve(std::path::Path::new(&p)) {
                                Ok(t) => this.action(
                                    "packages.install_development",
                                    json!({
                                        "package_id": t.id,
                                        "version": t.version,
                                        "archive_path": t.archive_path.to_string_lossy(),
                                    }),
                                ),
                                Err(e) => this.error = Some(e),
                            }
                            cx.notify();
                        },
                    )),
            ),
    );

    col = col.child(slot_or(app, "dev.packages", |v| {
        let items = varr(v, "packages");
        // Dev installs never come from the signed catalog — catalog_sequence
        // 0 is the marker (see packages.install_development).
        let dev: Vec<_> = items
            .iter()
            .filter(|p| super::store::is_development(p))
            .cloned()
            .collect();
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child(format!("Dev-пакеты ({})", dev.len())),
        );
        if dev.is_empty() {
            el = el.child(empty("Dev-пакеты не установлены"));
        }
        for p in dev {
            let id = vstr(&p, "id");
            let uid = id.clone();
            el = el.child(
                entry_row(
                    icon_file(vopt(&p, "icon_path")),
                    vopt(&p, "name").unwrap_or_else(|| id.clone()),
                    format!("{id} · v{}", vstr(&p, "version")),
                )
                .child(btn_id(&format!("dev-un-{id}"), "Удалить", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Удалить dev-пакет",
                            "Пакет будет удалён из Engine.",
                            "packages.uninstall",
                            json!({"package_id": uid}),
                            cx,
                        );
                    })
                })),
            );
        }
        el.into_any_element()
    }));
    col.into_any_element()
}
