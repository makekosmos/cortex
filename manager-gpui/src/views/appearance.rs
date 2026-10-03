//! Persistent appearance editor. Engine owns policy; controls never infer OS support.
use crate::app::{ManagerApp, Slot};
use crate::theme::*;
use crate::widgets::*;
use ::gpui::{prelude::*, *};
use gpui_component::Disableable;
use serde_json::{json, Value};

mod colors;
mod previews;
mod selector;
mod typography;

pub fn load(app: &mut ManagerApp) {
    app.call("appearance", "appearance.get", json!({}));
}

pub(super) fn editable(app: &ManagerApp) -> bool {
    app.appearance.ready && matches!(app.slots.get("appearance"), Some(Slot::Ready(_)))
}

pub(super) fn patch(app: &mut ManagerApp, params: Value, cx: &mut Context<ManagerApp>) {
    if !editable(app) {
        return;
    }
    // Keep the current page in place. A global action would show a bottom
    // banner and reload the view, which shifts the whole window.
    if let Some(Slot::Ready(value)) = app.slots.get_mut("appearance") {
        if let (Some(settings), Some(patch)) = (
            value.get_mut("settings").and_then(Value::as_object_mut),
            params.as_object(),
        ) {
            for (key, item) in patch {
                settings.insert(key.clone(), item.clone());
            }
        }
        let snapshot = value.clone();
        app.appearance.ingest(&snapshot);
    }
    app.refresh("appearance.set", "appearance.set", params);
    cx.notify();
}

pub(super) fn section_label(label: &'static str) -> Div {
    div()
        .px(px(8.))
        .text_size(px(13.))
        .line_height(px(17.))
        .text_color(c(MUTED_FG()))
        .child(label)
}

pub(super) fn section_block(label: &'static str, block: impl IntoElement) -> Div {
    div()
        .mt(px(32.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(section_label(label))
        .child(block)
}

pub(super) fn settings_card() -> Div {
    card().px(px(0.)).py(px(0.)).gap(px(0.))
}

pub(super) fn card_row(first: bool) -> Div {
    div()
        .mx(px(16.))
        .py(px(12.))
        .min_h(px(60.))
        .when(!first, |row| {
            row.border_t_1().border_color(fade(BORDER(), 0.6))
        })
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(16.))
}

pub(super) fn row_title(title: &'static str) -> Div {
    div()
        .min_w_0()
        .text_size(px(13.))
        .line_height(px(17.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(FG()))
        .child(title)
}

pub(super) fn row_meta(text: &str) -> Div {
    div()
        .mt(px(2.))
        .min_w_0()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(fade(MUTED_FG(), 0.65))
        .child(text.to_owned())
}

pub(super) fn watched_input(
    app: &mut ManagerApp,
    key: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Entity<gpui_component::input::InputState> {
    let new = !app.inputs.contains_key(key);
    let input = app.input(key, placeholder.to_owned(), false, window, cx);
    if new {
        cx.subscribe(&input, |_, _, _: &gpui_component::input::InputEvent, cx| {
            cx.notify()
        })
        .detach();
    }
    input
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let status = match app.slots.get("appearance") {
        Some(Slot::Ready(_)) => "",
        Some(Slot::Failed(_)) => {
            "Настройки внешнего вида недоступны. Сохранённый стиль не изменён."
        }
        _ => "Загрузка настроек внешнего вида…",
    };
    let enabled = editable(app);
    let wallpaper = app.appearance.ready && app.appearance.settings.accent_source == "wallpaper";
    let wallpaper_control = if app.appearance.ready {
        toggle(
            "wallpaper-theme-colors",
            wallpaper,
            cx,
            |app, enabled, cx| {
                patch(
                    app,
                    if enabled {
                        json!({"accent_source":"wallpaper"})
                    } else {
                        json!({"accent_source":"theme","accent_color":Value::Null})
                    },
                    cx,
                );
            },
        )
        .accessibility_label("Цвета обоев")
        .disabled(!enabled || !app.appearance.wallpaper_supported)
        .into_any_element()
    } else {
        badge("Загрузка…", MUTED_FG()).into_any_element()
    };
    let follow = app.appearance.settings.follow_apps;
    let follow_control = if app.appearance.ready {
        toggle("appearance-follow-apps", follow, cx, |app, follow, cx| {
            patch(app, json!({"follow_apps":follow}), cx);
        })
        .accessibility_label("Единый стиль приложений")
        .disabled(!enabled)
        .into_any_element()
    } else {
        badge("Загрузка…", MUTED_FG()).into_any_element()
    };
    let material = app.appearance.settings.material.clone();
    let material_options = [
        ("default", "Цвета темы"),
        ("frosted", "Матовое стекло"),
        ("opaque", "Непрозрачный"),
        ("acrylic", "Acrylic"),
        ("mica", "Mica"),
    ]
    .into_iter()
    .filter(|(key, _)| app.appearance.materials.iter().any(|item| item == key))
    .map(|(key, label)| selector::OptionItem::new(label, key))
    .collect();
    let material_select = selector::select(
        app,
        selector::SelectSpec {
            kind: crate::appearance_state::AppearanceMenu::Material,
            id: "appearance-surface",
            aria_label: "Стекло",
            current: material.clone(),
            options: material_options,
            trigger_width: 148.,
            menu_width: 160.,
            heading: None,
        },
        window,
        cx,
    );
    let color_card = settings_card()
        .id("appearance-theme-card")
        .debug_selector(|| "appearance-theme-card".into())
        .children(previews::theme_rows(app, window, cx))
        .child(
            card_row(false)
                .debug_selector(|| "appearance-accent-card".into())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(160.))
                        .child(row_title("Акцентный цвет"))
                        .child(row_meta(if wallpaper {
                            "Цвета обоев включены; этот акцент используется, когда они выключены."
                        } else {
                            "Цвет темы или один из образцов."
                        })),
                )
                .child(colors::accent_controls(app, cx)),
        )
        .child(
            card_row(false)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(160.))
                        .child(row_title("Цвета обоев"))
                        .child(row_meta(colors::wallpaper_meta(app))),
                )
                .child(wallpaper_control),
        );
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(section("Внешний вид", "Темы, цвет, материал и шрифт"))
        .when(!status.is_empty(), |page| page.child(empty(status)))
        .child(
            div()
                .mt(px(24.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(section_label("Цветовая схема"))
                .child(previews::modes(app, cx)),
        )
        .child(color_card.mt(px(16.)))
        .child(section_block(
            "Материал",
            settings_card()
                .id("appearance-material-card")
                .debug_selector(|| "appearance-material-card".into())
                .child(
                    card_row(true)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(160.))
                                .child(row_title("Стекло"))
                                .child(row_meta(match material.as_str() {
                                    "frosted" | "acrylic" | "mica" => "Прозрачные поверхности.",
                                    "opaque" => "Сплошные поверхности.",
                                    _ => "Цвета темы: сплошные поверхности.",
                                })),
                        )
                        .child(material_select),
                )
                .child(
                    card_row(false)
                        .id("appearance-apps-card")
                        .debug_selector(|| "appearance-apps-card".into())
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(160.))
                                .child(row_title("Единый стиль приложений"))
                                .child(row_meta(
                                    "Agenda использует этот стиль. Остальным нужен API Engine.",
                                )),
                        )
                        .child(follow_control),
                ),
        ))
        .child(typography::render(app, window, cx))
        .into_any_element()
}
